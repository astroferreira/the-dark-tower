//! Main simulation state and tick loop

use std::collections::HashMap;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::world::WorldData;
use crate::simulation::types::{
    TribeId, TileCoord, SimTick, RelationLevel, TribeEvent, TribeEventType, Treaty,
    GlobalLocalCoord, LOCAL_MAP_SIZE,
};
use crate::simulation::params::SimulationParams;
use crate::simulation::tribe::{Tribe, TribeCulture, Settlement};
use crate::simulation::tribe::culture::generate_tribe_name;
use crate::simulation::society::{SocietyType, process_succession};
use crate::simulation::colonists::{
    process_notable_lifecycle, process_notable_births,
    promote_to_notable, target_notable_count, process_colonist_movement, Colonist,
};
use crate::simulation::jobs::{assign_all_jobs, process_jobs};
use crate::simulation::resources::extract_resources;
use crate::simulation::technology::BuildingType;
use crate::simulation::interaction::{DiplomacyState, process_diplomacy_tick, process_trade_tick, process_conflict_tick, ReputationState};
use crate::simulation::territory::process_expansion_tick;
use crate::simulation::structures::{StructureManager, StructureType};
use crate::simulation::roads::{RoadNetwork, RoadType};
use crate::simulation::monsters::{MonsterManager, MonsterId, Monster};
use crate::simulation::fauna::{FaunaManager, FaunaId, Fauna};
use crate::simulation::characters::CharacterManager;
use crate::simulation::combat::CombatLogStore;

/// Radius in local tiles for full simulation detail
pub const FOCUS_RADIUS_FULL: u32 = 100;
/// Radius in local tiles for medium simulation detail
pub const FOCUS_RADIUS_MEDIUM: u32 = 300;
/// How often to update sparse entities (every N ticks)
pub const SPARSE_UPDATE_INTERVAL: u64 = 4;

/// Main simulation state
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SimulationState {
    /// Current simulation tick
    pub current_tick: SimTick,
    /// All tribes in the simulation
    pub tribes: HashMap<TribeId, Tribe>,
    /// Diplomatic relations and treaties
    pub diplomacy: DiplomacyState,
    /// Reputation between tribes and monster species
    pub reputation: ReputationState,
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
    /// Fauna manager
    pub fauna: FaunaManager,
    /// Character manager for detailed combat
    #[serde(skip)]
    pub character_manager: CharacterManager,
    /// Combat log store
    #[serde(skip)]
    pub combat_log: CombatLogStore,
    /// Focus point for detailed simulation (camera position)
    #[serde(skip)]
    pub focus_point: Option<GlobalLocalCoord>,
    /// World dimensions for distance calculations
    #[serde(skip)]
    pub world_width: usize,
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
    pub total_fauna_spawned: u32,
    pub total_fauna_hunted: u32,
    pub current_fauna_count: u32,
}

impl SimulationState {
    /// Create a new simulation state
    pub fn new(seed: u64) -> Self {
        SimulationState {
            current_tick: SimTick(0),
            tribes: HashMap::new(),
            diplomacy: DiplomacyState::new(),
            reputation: ReputationState::new(),
            territory_map: HashMap::new(),
            next_tribe_id: 0,
            stats: SimulationStats::default(),
            seed,
            structures: StructureManager::new(),
            road_network: RoadNetwork::new(),
            monsters: MonsterManager::new(),
            fauna: FaunaManager::new(),
            character_manager: CharacterManager::new(),
            combat_log: CombatLogStore::new(),
            focus_point: None,
            world_width: 512, // Will be set properly in initialize
        }
    }

    /// Set the focus point for detailed simulation (usually camera position)
    pub fn set_focus(&mut self, focus: GlobalLocalCoord) {
        self.focus_point = Some(focus);
    }

    /// Check if a position is within full simulation range
    pub fn is_in_focus(&self, pos: &GlobalLocalCoord) -> bool {
        match self.focus_point {
            Some(focus) => {
                pos.distance_wrapped(&focus, self.world_width) <= FOCUS_RADIUS_FULL
            }
            None => true, // If no focus, everything is in focus
        }
    }

    /// Check if a position is within medium simulation range
    pub fn is_in_medium_range(&self, pos: &GlobalLocalCoord) -> bool {
        match self.focus_point {
            Some(focus) => {
                pos.distance_wrapped(&focus, self.world_width) <= FOCUS_RADIUS_MEDIUM
            }
            None => true,
        }
    }

    /// Check if a world tile is within focus range
    pub fn is_tile_in_focus(&self, tile: &TileCoord) -> bool {
        let pos = GlobalLocalCoord::from_world_tile(*tile);
        self.is_in_medium_range(&pos)
    }

    /// Check if this tick should update sparse entities
    pub fn should_update_sparse(&self) -> bool {
        self.current_tick.0 % SPARSE_UPDATE_INTERVAL == 0
    }

    /// Initialize the simulation with tribes placed on the world
    pub fn initialize<R: Rng>(
        &mut self,
        world: &WorldData,
        params: &SimulationParams,
        rng: &mut R,
    ) {
        // Set world dimensions for focus calculations
        self.world_width = world.width;

        // Find suitable spawn locations
        let spawn_locations = self.find_spawn_locations(world, params, rng);

        // Create tribes at each location
        for location in spawn_locations {
            let biome = *world.biomes.get(location.x, location.y);
            let culture = TribeCulture::from_biome(biome, rng);
            let name = generate_tribe_name(&culture, biome, rng);

            // Determine society type based on culture and location
            let is_coastal = world.is_coastal(location.x, location.y);
            let is_warlike = culture.is_warlike();
            let society_type = SocietyType::random_weighted(rng, is_coastal, is_warlike);

            let tribe_id = self.create_tribe_with_society(
                name,
                location,
                params.initial_tribe_population,
                culture,
                society_type,
                rng,
            );

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

    /// Create a new tribe (basic version, uses default society)
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

    /// Create a new tribe with a specific society type and initialized colonists
    fn create_tribe_with_society<R: Rng>(
        &mut self,
        name: String,
        capital: TileCoord,
        population: u32,
        culture: TribeCulture,
        society_type: SocietyType,
        rng: &mut R,
    ) -> TribeId {
        let id = TribeId(self.next_tribe_id);
        self.next_tribe_id += 1;

        let tribe = Tribe::new_with_society(
            id,
            name,
            capital,
            population,
            culture,
            society_type,
            self.current_tick.0,
            rng,
        );
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
        // 1. Colony Lifecycle (NEW) - aging, birth, death for notable colonists
        self.process_colony_lifecycle(params, rng);

        // 2. Job Assignment (NEW) - assign workers to jobs
        self.process_job_assignment(world, params);

        // 3. Job Processing (NEW) - execute jobs, produce resources
        self.process_job_work(world, params, rng);

        // 3.5. Colonist Movement - move colonists based on their jobs
        self.process_colonist_movement(world, rng);

        // 4. Resource Consumption
        self.process_resource_consumption(params);

        // 5. Colonist Needs (NEW) - mood updates
        self.process_colonist_needs(params);

        // 6. Society Updates (NEW) - elections, succession
        self.process_society_updates(params, rng);

        // 7. Needs Calculation
        self.process_needs_calculation(params);

        // 8. Population Dynamics (uses pool now)
        self.process_population_dynamics(params);

        // 9. Technology Progress
        self.process_technology_progress(params);

        // 10. Territory Management
        process_expansion_tick(self, world, params, rng);

        // 11. Monster Processing
        self.process_monsters(world, params, rng);

        // 11.5. Fauna Processing
        self.process_fauna(world, params, rng);

        // 12. Structure and Road Updates
        self.process_structures_and_roads(world, params, rng);

        // 13. Diplomacy Updates
        process_diplomacy_tick(self, params, rng);

        // 14. Trade
        process_trade_tick(self, params, rng);

        // 15. Conflict
        process_conflict_tick(self, params, rng);

        // 16. Cleanup
        self.cleanup_extinct_tribes();
        self.monsters.cleanup_dead();
        self.fauna.cleanup_dead();

        // Advance tick
        self.current_tick = self.current_tick.next();
        self.update_stats();
    }

    /// Process colony lifecycle for all tribes (notable colonist aging, births, deaths)
    fn process_colony_lifecycle<R: Rng>(&mut self, params: &SimulationParams, rng: &mut R) {
        let current_tick = self.current_tick.0;

        for tribe in self.tribes.values_mut() {
            if !tribe.is_alive {
                continue;
            }

            // Process notable colonist lifecycle
            let health_satisfaction = tribe.needs.health.satisfaction;
            let lifecycle_result = process_notable_lifecycle(
                &mut tribe.notable_colonists,
                current_tick,
                health_satisfaction,
                rng,
            );

            // Process pool population dynamics
            let food_satisfaction = tribe.needs.food.satisfaction;
            let _pool_result = tribe.population_pool.tick(
                food_satisfaction,
                health_satisfaction,
                params.base_growth_rate,
                params.base_death_rate,
                rng,
            );

            // Process notable births
            let _births = process_notable_births(
                &mut tribe.notable_colonists,
                current_tick,
                food_satisfaction,
                params.base_growth_rate,
                rng,
            );

            // Ensure we have enough notables (promote from pool if needed)
            let target_notables = target_notable_count(tribe.population.total());
            let current_notables = tribe.notable_colonists.count();
            if current_notables < target_notables {
                let to_promote = target_notables - current_notables;
                let capital = tribe.capital;
                for _ in 0..to_promote.min(2) {
                    promote_to_notable(
                        &mut tribe.notable_colonists,
                        &mut tribe.population_pool,
                        crate::simulation::colonists::ColonistRole::Citizen,
                        current_tick,
                        capital,
                        rng,
                    );
                }
            }
        }
    }

    /// Process job assignment for all tribes
    fn process_job_assignment(&mut self, world: &WorldData, params: &SimulationParams) {
        for tribe in self.tribes.values_mut() {
            if !tribe.is_alive {
                continue;
            }

            // Determine environmental factors for job demand
            let has_mines = tribe.territory.iter().any(|coord| {
                let biome = world.biomes.get(coord.x, coord.y);
                matches!(biome,
                    crate::biomes::ExtendedBiome::AlpineTundra |
                    crate::biomes::ExtendedBiome::SnowyPeaks |
                    crate::biomes::ExtendedBiome::RazorPeaks |
                    crate::biomes::ExtendedBiome::Foothills
                )
            });
            let has_forests = tribe.territory.iter().any(|coord| {
                let biome = world.biomes.get(coord.x, coord.y);
                matches!(biome,
                    crate::biomes::ExtendedBiome::TemperateForest |
                    crate::biomes::ExtendedBiome::TropicalForest |
                    crate::biomes::ExtendedBiome::BorealForest |
                    crate::biomes::ExtendedBiome::TropicalRainforest
                )
            });
            let has_water = tribe.territory.iter().any(|coord| {
                let biome = world.biomes.get(coord.x, coord.y);
                matches!(biome,
                    crate::biomes::ExtendedBiome::FrozenLake |
                    crate::biomes::ExtendedBiome::CoastalWater |
                    crate::biomes::ExtendedBiome::MirrorLake |
                    crate::biomes::ExtendedBiome::Cenote |
                    crate::biomes::ExtendedBiome::Lagoon
                )
            });

            // Update job demand
            tribe.jobs.update_demand(
                tribe.population.total(),
                tribe.needs.food.satisfaction,
                tribe.needs.security.satisfaction,
                has_mines,
                has_forests,
                has_water,
                false, // TODO: check if at war
                tribe.count_buildings("") < 5, // needs buildings if less than 5
                tribe.society_state.society_type,
            );

            // Assign workers to jobs
            assign_all_jobs(
                &mut tribe.jobs,
                &mut tribe.notable_colonists,
                &mut tribe.population_pool,
                tribe.society_state.society_type,
            );
        }
    }

    /// Process job work and produce resources
    fn process_job_work<R: Rng>(&mut self, world: &WorldData, params: &SimulationParams, rng: &mut R) {
        let season_modifier = self.current_tick.season().food_modifier();

        for tribe in self.tribes.values_mut() {
            if !tribe.is_alive {
                continue;
            }

            let result = process_jobs(
                &tribe.jobs,
                &mut tribe.notable_colonists,
                &mut tribe.population_pool,
                &tribe.stockpile,
                &tribe.society_state,
                season_modifier,
                rng,
            );

            // Apply production to stockpile
            result.apply_to_stockpile(&mut tribe.stockpile);

            // Apply research points
            tribe.tech_state.add_research(result.research_points);
        }
    }

    /// Process colonist movement for all tribes
    /// Focused tribes get detailed simulation, distant tribes get sparse updates
    fn process_colonist_movement<R: Rng>(&mut self, world: &WorldData, rng: &mut R) {
        let current_tick = self.current_tick.0;
        let focus_point = self.focus_point;
        let world_width = self.world_width;
        let should_update_sparse = self.should_update_sparse();

        for tribe in self.tribes.values_mut() {
            if !tribe.is_alive {
                continue;
            }

            // Check if this tribe is in focus
            let tribe_in_focus = match focus_point {
                Some(focus) => {
                    tribe.city_center.distance_wrapped(&focus, world_width) <= FOCUS_RADIUS_MEDIUM
                }
                None => true,
            };

            // Skip distant tribes on non-sparse ticks
            if !tribe_in_focus && !should_update_sparse {
                continue;
            }

            process_colonist_movement(
                &mut tribe.notable_colonists.colonists,
                &tribe.territory,
                tribe.capital,
                world,
                current_tick,
                tribe_in_focus,
                rng,
            );

            // Update the spatial index after movement
            tribe.notable_colonists.update_colonist_map();
        }
    }

    /// Process colonist needs and mood
    fn process_colonist_needs(&mut self, _params: &SimulationParams) {
        for tribe in self.tribes.values_mut() {
            if !tribe.is_alive {
                continue;
            }

            // Cache values before mutable borrow
            let food_sat = tribe.needs.food.satisfaction;
            let shelter_sat = tribe.needs.shelter.satisfaction;
            let security_sat = tribe.needs.security.satisfaction;
            let has_social = tribe.notable_colonists.count() > 1;

            // Update mood for notable colonists based on needs
            for colonist in tribe.notable_colonists.colonists.values_mut() {
                if !colonist.is_alive {
                    continue;
                }

                crate::simulation::colonists::apply_needs_modifiers(
                    &mut colonist.mood,
                    food_sat,
                    shelter_sat,
                    security_sat,
                    has_social,
                );
            }
        }
    }

    /// Process society updates (succession, unrest, etc.)
    fn process_society_updates<R: Rng>(&mut self, params: &SimulationParams, rng: &mut R) {
        for tribe in self.tribes.values_mut() {
            if !tribe.is_alive {
                continue;
            }

            // Tick society state
            tribe.society_state.tick();

            // Check for leader death
            if crate::simulation::society::succession::check_leader_death(&tribe.society_state, rng) {
                tribe.society_state.trigger_succession();
            }

            // Process succession if in crisis
            if tribe.society_state.in_succession_crisis {
                let notable_names = tribe.notable_colonists.notable_names();
                process_succession(&mut tribe.society_state, &notable_names, rng);
            }

            // Add unrest from low morale
            let avg_morale = tribe.needs.morale.satisfaction;
            if avg_morale < 0.3 {
                tribe.society_state.add_unrest(0.05);
            }

            // Check for revolution
            if tribe.society_state.should_revolt() {
                // Revolution! Change society type
                let new_type = if tribe.society_state.society_type == SocietyType::MilitaryDictatorship {
                    SocietyType::Democracy // Military dictatorships often become democracies
                } else {
                    SocietyType::MilitaryDictatorship // Other revolutions often become dictatorships
                };
                tribe.society_state.society_type = new_type;
                tribe.society_state.revolution_progress = 0.0;
                tribe.society_state.trigger_succession();
            }
        }
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
        self.stats.current_fauna_count = self.fauna.living_count() as u32;
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

        // Process monster behavior (movement, state changes), considering reputation
        // Pass focus information for sparse simulation
        self.monsters.process_behavior(
            &self.tribes,
            &self.territory_map,
            &self.reputation,
            world,
            current_tick,
            self.focus_point,
            self.world_width,
            rng,
        );

        // Process combat - use detailed combat for significant monsters
        self.process_monster_combat_detailed(world, rng);

        // Decay reputation once per year (every 4 ticks)
        if current_tick % 4 == 0 {
            self.reputation.process_decay();
        }

        // Update current monster count
        self.stats.current_monster_count = self.monsters.living_count() as u32;
    }

    /// Process fauna spawning and behavior
    fn process_fauna<R: Rng>(&mut self, world: &WorldData, _params: &SimulationParams, rng: &mut R) {
        let current_tick = self.current_tick.0;

        // Spawn fauna periodically
        if current_tick % self.fauna.spawn_params.spawn_interval == 0 {
            if let Some(_fauna_id) = self.fauna.try_spawn(world, &self.territory_map, current_tick, rng) {
                self.stats.total_fauna_spawned += 1;
            }
        }

        // Process fauna behavior (movement, grazing, fleeing, breeding)
        self.fauna.process_behavior(
            &self.tribes,
            &self.territory_map,
            &self.monsters.monsters,
            world,
            current_tick,
            self.focus_point,
            self.world_width,
            rng,
        );

        // Update current fauna count
        self.stats.current_fauna_count = self.fauna.living_count() as u32;
    }

    /// Process monster combat with detailed combat for significant monsters
    fn process_monster_combat_detailed<R: Rng>(&mut self, world: &WorldData, rng: &mut R) {
        use crate::simulation::monsters::{
            AttackTarget, MonsterState, CombatEvent, is_significant_monster,
            run_detailed_monster_vs_tribe_combat, run_detailed_monster_vs_monster_combat,
            calculate_reputation_change,
        };

        // Collect attacking monsters
        let attacking_monsters: Vec<_> = self.monsters.monsters
            .iter()
            .filter_map(|(id, m)| {
                if let MonsterState::Attacking(target) = m.state {
                    Some((*id, target, is_significant_monster(m), m.location))
                } else {
                    None
                }
            })
            .collect();

        for (monster_id, target, is_significant, location) in attacking_monsters {
            match target {
                AttackTarget::Tribe(tribe_id) => {
                    if is_significant {
                        // Use detailed combat for significant monsters
                        let monster = match self.monsters.monsters.get(&monster_id) {
                            Some(m) if !m.is_dead() => m.clone(),
                            _ => continue,
                        };
                        let tribe = match self.tribes.get(&tribe_id) {
                            Some(t) if t.is_alive => t.clone(),
                            _ => continue,
                        };

                        // Check if in range
                        let in_range = tribe.territory.iter().any(|coord| {
                            monster.distance_to(coord, world.width) <= 2
                        });
                        if !in_range {
                            continue;
                        }

                        let current_tick = self.current_tick.0;

                        // Run detailed combat
                        let (monster_damage, casualties, monster_killed, _entries) =
                            run_detailed_monster_vs_tribe_combat(
                                &monster,
                                &tribe,
                                &mut self.character_manager,
                                &mut self.combat_log,
                                Some(location),
                                current_tick,
                                rng,
                            );

                        // Apply results
                        if let Some(m) = self.monsters.monsters.get_mut(&monster_id) {
                            m.health -= monster_damage;
                            if monster_killed || m.health <= 0.0 {
                                m.state = MonsterState::Dead;
                                self.stats.total_monsters_killed += 1;
                            } else if m.should_flee() {
                                m.state = MonsterState::Fleeing;
                            }
                            m.kills += casualties;
                        }

                        if casualties > 0 {
                            if let Some(t) = self.tribes.get_mut(&tribe_id) {
                                if t.population.total() > casualties {
                                    t.population.apply_casualties(casualties);
                                } else {
                                    t.is_alive = false;
                                }
                            }
                        }

                        // Update reputation based on combat outcome
                        let rep_change = calculate_reputation_change(monster_killed, monster_damage, &monster);
                        self.reputation.adjust(tribe_id, monster.species, rep_change);

                        self.stats.total_monster_attacks += 1;
                    } else {
                        // Use simple combat for regular monsters
                        self.process_simple_monster_vs_tribe_combat(monster_id, tribe_id, world, rng);
                    }
                }
                AttackTarget::Monster(target_id) => {
                    let attacker = match self.monsters.monsters.get(&monster_id) {
                        Some(m) if !m.is_dead() => m.clone(),
                        _ => continue,
                    };
                    let defender = match self.monsters.monsters.get(&target_id) {
                        Some(m) if !m.is_dead() => m.clone(),
                        _ => continue,
                    };

                    if is_significant || is_significant_monster(&defender) {
                        // Use detailed combat
                        let current_tick = self.current_tick.0;

                        let (attacker_damage, defender_damage, attacker_killed, defender_killed, _entries) =
                            run_detailed_monster_vs_monster_combat(
                                &attacker,
                                &defender,
                                &mut self.character_manager,
                                &mut self.combat_log,
                                Some(location),
                                current_tick,
                                rng,
                            );

                        // Apply results to attacker
                        if let Some(m) = self.monsters.monsters.get_mut(&monster_id) {
                            m.health -= attacker_damage;
                            if attacker_killed || m.health <= 0.0 {
                                m.state = MonsterState::Dead;
                                self.stats.total_monsters_killed += 1;
                            } else if m.should_flee() {
                                m.state = MonsterState::Fleeing;
                            }
                            if defender_killed {
                                m.kills += 1;
                            }
                        }

                        // Apply results to defender
                        if let Some(m) = self.monsters.monsters.get_mut(&target_id) {
                            m.health -= defender_damage;
                            if defender_killed || m.health <= 0.0 {
                                m.state = MonsterState::Dead;
                                self.stats.total_monsters_killed += 1;
                            } else if m.should_flee() {
                                m.state = MonsterState::Fleeing;
                            }
                            if attacker_killed {
                                m.kills += 1;
                            }
                        }

                        self.stats.total_monster_attacks += 1;
                    } else {
                        // Use simple combat
                        self.process_simple_monster_vs_monster_combat(monster_id, target_id, rng);
                    }
                }
            }
        }

        // Clean up dead monsters
        self.monsters.cleanup_dead();
    }

    /// Simple combat for regular monsters vs tribes
    fn process_simple_monster_vs_tribe_combat<R: Rng>(
        &mut self,
        monster_id: crate::simulation::monsters::MonsterId,
        tribe_id: TribeId,
        world: &WorldData,
        rng: &mut R,
    ) {
        use crate::simulation::monsters::{MonsterState, calculate_reputation_change};
        use crate::simulation::combat::{
            CombatLogEntry, CombatAction, CombatResult, CombatantRef, EncounterOutcome,
        };

        let monster = match self.monsters.monsters.get(&monster_id) {
            Some(m) if !m.is_dead() => m,
            _ => return,
        };
        let tribe = match self.tribes.get(&tribe_id) {
            Some(t) if t.is_alive => t,
            _ => return,
        };

        // Check if in range
        let in_range = tribe.territory.iter().any(|coord| {
            monster.distance_to(coord, world.width) <= 2
        });
        if !in_range {
            return;
        }

        // Capture monster info for reputation update before combat changes state
        let monster_species = monster.species;
        let monster_for_rep = monster.clone();

        let monster_name = format!("{} #{}", monster.species.name(), monster_id.0);
        let tribe_name = tribe.name.clone();
        let monster_location = monster.location;
        let monster_strength = monster.strength;
        let tribe_defense = tribe.military_strength() * 0.5;

        let monster_roll = rng.gen::<f32>() * monster_strength;
        let tribe_roll = rng.gen::<f32>() * tribe_defense;

        let (monster_damage, tribe_casualties) = if monster_roll > tribe_roll {
            let damage_ratio = (monster_roll - tribe_roll) / monster_strength;
            let casualties = ((tribe.population.total() as f32 * damage_ratio * 0.05) as u32).max(1).min(20);
            ((tribe_defense * rng.gen::<f32>() * 0.3).max(1.0), casualties)
        } else {
            ((tribe_defense * rng.gen::<f32>() * 0.5).max(2.0), 0)
        };

        let current_tick = self.current_tick.0;
        let monster_killed;

        // Apply damage to monster
        if let Some(m) = self.monsters.monsters.get_mut(&monster_id) {
            let killed = m.take_damage(monster_damage);
            monster_killed = killed;
            if killed {
                m.state = MonsterState::Dead;
                self.stats.total_monsters_killed += 1;
            } else if m.should_flee() {
                m.state = MonsterState::Fleeing;
            }
            m.kills += tribe_casualties;
        } else {
            monster_killed = false;
        }

        // Apply casualties to tribe
        if tribe_casualties > 0 {
            if let Some(t) = self.tribes.get_mut(&tribe_id) {
                if t.population.total() > tribe_casualties {
                    t.population.apply_casualties(tribe_casualties);
                } else {
                    t.is_alive = false;
                }
            }
        }

        // Create a simple combat log entry
        let encounter_id = self.combat_log.start_encounter(current_tick, Some(monster_location));

        let result = if monster_killed {
            CombatResult::Kill { cause: "combat wounds".to_string() }
        } else if tribe_casualties > 0 {
            CombatResult::Wound
        } else {
            CombatResult::Miss
        };

        let narrative = if monster_killed {
            format!("{} defenders killed the {} in battle", tribe_name, monster_name)
        } else if tribe_casualties > 0 {
            format!("{} attacked {}, killing {} defenders", monster_name, tribe_name, tribe_casualties)
        } else {
            format!("{} attacked {} but was driven back", monster_name, tribe_name)
        };

        let entry = CombatLogEntry {
            tick: current_tick,
            attacker: CombatantRef {
                id: monster_id.0 as u64,
                name: monster_name.clone(),
                faction: "Monster".to_string(),
            },
            defender: CombatantRef {
                id: tribe_id.0 as u64,
                name: tribe_name.clone(),
                faction: tribe_name.clone(),
            },
            action: CombatAction::Attack {
                weapon: "natural attack".to_string(),
                damage_type: "blunt".to_string(),
            },
            target_part: None,
            damage: Some(monster_damage),
            wound_type: None,
            wound_severity: None,
            result,
            effects: vec![],
            narrative,
        };

        self.combat_log.add_entry_to_encounter(encounter_id, entry);

        let outcome = if monster_killed {
            EncounterOutcome::Victory { winner: tribe_name }
        } else if tribe_casualties > 0 {
            EncounterOutcome::Victory { winner: monster_name }
        } else {
            EncounterOutcome::Fled { fleeing_party: monster_name }
        };
        self.combat_log.end_encounter(encounter_id, current_tick, outcome);

        // Update reputation based on combat outcome
        let rep_change = calculate_reputation_change(monster_killed, monster_damage, &monster_for_rep);
        self.reputation.adjust(tribe_id, monster_species, rep_change);

        self.stats.total_monster_attacks += 1;
    }

    /// Simple combat for regular monsters vs monsters
    fn process_simple_monster_vs_monster_combat<R: Rng>(
        &mut self,
        attacker_id: crate::simulation::monsters::MonsterId,
        defender_id: crate::simulation::monsters::MonsterId,
        rng: &mut R,
    ) {
        use crate::simulation::monsters::MonsterState;
        use crate::simulation::combat::{
            CombatLogEntry, CombatAction, CombatResult, CombatantRef, EncounterOutcome,
        };

        let (attacker_strength, attacker_name, attacker_location) = match self.monsters.monsters.get(&attacker_id) {
            Some(m) if !m.is_dead() => (m.strength, format!("{} #{}", m.species.name(), attacker_id.0), m.location),
            _ => return,
        };
        let (defender_strength, defender_name) = match self.monsters.monsters.get(&defender_id) {
            Some(m) if !m.is_dead() => (m.strength, format!("{} #{}", m.species.name(), defender_id.0)),
            _ => return,
        };

        let attacker_roll = rng.gen::<f32>() * attacker_strength;
        let defender_roll = rng.gen::<f32>() * defender_strength;

        let (attacker_damage, defender_damage) = if attacker_roll > defender_roll {
            (defender_strength * 0.1 * rng.gen::<f32>(), attacker_strength * 0.3 * rng.gen::<f32>())
        } else {
            (defender_strength * 0.3 * rng.gen::<f32>(), attacker_strength * 0.1 * rng.gen::<f32>())
        };

        // Apply damage to attacker
        let attacker_killed = {
            let m = self.monsters.monsters.get_mut(&attacker_id).unwrap();
            let killed = m.take_damage(attacker_damage);
            if killed {
                m.state = MonsterState::Dead;
                self.stats.total_monsters_killed += 1;
            } else if m.should_flee() {
                m.state = MonsterState::Fleeing;
            }
            killed
        };

        // Apply damage to defender
        let defender_killed = {
            let m = self.monsters.monsters.get_mut(&defender_id).unwrap();
            let killed = m.take_damage(defender_damage);
            if killed {
                m.state = MonsterState::Dead;
                self.stats.total_monsters_killed += 1;
            } else if m.should_flee() {
                m.state = MonsterState::Fleeing;
            }
            killed
        };

        // Update kill counts
        if defender_killed {
            if let Some(m) = self.monsters.monsters.get_mut(&attacker_id) {
                m.kills += 1;
            }
        }
        if attacker_killed {
            if let Some(m) = self.monsters.monsters.get_mut(&defender_id) {
                m.kills += 1;
            }
        }

        // Create combat log entry
        let current_tick = self.current_tick.0;
        let encounter_id = self.combat_log.start_encounter(current_tick, Some(attacker_location));

        let result = if attacker_killed && defender_killed {
            CombatResult::Kill { cause: "mutual wounds".to_string() }
        } else if defender_killed {
            CombatResult::Kill { cause: "combat wounds".to_string() }
        } else if defender_damage > 5.0 {
            CombatResult::Wound
        } else {
            CombatResult::Hit
        };

        let narrative = if attacker_killed && defender_killed {
            format!("{} and {} killed each other in battle", attacker_name, defender_name)
        } else if attacker_killed {
            format!("{} was slain by {} in combat", attacker_name, defender_name)
        } else if defender_killed {
            format!("{} killed {} in combat", attacker_name, defender_name)
        } else {
            format!("{} fought {} - both survived", attacker_name, defender_name)
        };

        let entry = CombatLogEntry {
            tick: current_tick,
            attacker: CombatantRef {
                id: attacker_id.0 as u64,
                name: attacker_name.clone(),
                faction: "Monster".to_string(),
            },
            defender: CombatantRef {
                id: defender_id.0 as u64,
                name: defender_name.clone(),
                faction: "Monster".to_string(),
            },
            action: CombatAction::Attack {
                weapon: "natural attack".to_string(),
                damage_type: "slash".to_string(),
            },
            target_part: None,
            damage: Some(defender_damage),
            wound_type: None,
            wound_severity: None,
            result,
            effects: vec![],
            narrative,
        };

        self.combat_log.add_entry_to_encounter(encounter_id, entry);

        let outcome = if attacker_killed && defender_killed {
            EncounterOutcome::Mutual
        } else if attacker_killed {
            EncounterOutcome::Victory { winner: defender_name }
        } else if defender_killed {
            EncounterOutcome::Victory { winner: attacker_name }
        } else {
            EncounterOutcome::Fled { fleeing_party: "both".to_string() }
        };
        self.combat_log.end_encounter(encounter_id, current_tick, outcome);

        self.stats.total_monster_attacks += 1;
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

    /// Get colonist at a coordinate, returning the colonist and their tribe
    pub fn get_colonist_at(&self, coord: &TileCoord) -> Option<(&Colonist, TribeId)> {
        for (tribe_id, tribe) in &self.tribes {
            if let Some(colonist) = tribe.notable_colonists.get_at(coord) {
                return Some((colonist, *tribe_id));
            }
        }
        None
    }

    /// Get fauna at a coordinate
    pub fn get_fauna_at(&self, coord: &TileCoord) -> Vec<&Fauna> {
        self.fauna.get_at(coord)
    }

    /// Hunt fauna at a location
    pub fn hunt_fauna_at<R: Rng>(&mut self, coord: &TileCoord, hunting_skill: f32, rng: &mut R) -> (f32, f32) {
        let (food, materials) = self.fauna.hunt_at(coord, hunting_skill, rng);
        if food > 0.0 {
            self.stats.total_fauna_hunted += 1;
        }
        (food, materials)
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
