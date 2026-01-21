//! Trade networks and resource sites
//!
//! Generates trade routes between economic centers and places resource sites.

use std::collections::{HashMap, BinaryHeap, HashSet};
use std::cmp::Ordering;

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;
use crate::water_bodies::WaterBodyId;

use super::territories::{Settlement, TerritoryRegistry};
use super::types::*;

/// Type of resource
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ResourceType {
    Iron,
    Gold,
    Silver,
    Copper,
    Coal,
    Gems,
    Stone,
    Timber,
    Food,
    Salt,
    Spices,
    Furs,
}

impl ResourceType {
    pub fn all() -> &'static [ResourceType] {
        &[
            ResourceType::Iron,
            ResourceType::Gold,
            ResourceType::Silver,
            ResourceType::Copper,
            ResourceType::Coal,
            ResourceType::Gems,
            ResourceType::Stone,
            ResourceType::Timber,
            ResourceType::Food,
            ResourceType::Salt,
            ResourceType::Spices,
            ResourceType::Furs,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            ResourceType::Iron => "Iron",
            ResourceType::Gold => "Gold",
            ResourceType::Silver => "Silver",
            ResourceType::Copper => "Copper",
            ResourceType::Coal => "Coal",
            ResourceType::Gems => "Gems",
            ResourceType::Stone => "Stone",
            ResourceType::Timber => "Timber",
            ResourceType::Food => "Food",
            ResourceType::Salt => "Salt",
            ResourceType::Spices => "Spices",
            ResourceType::Furs => "Furs",
        }
    }

    /// Value multiplier for trade
    pub fn value(&self) -> f32 {
        match self {
            ResourceType::Gold => 10.0,
            ResourceType::Gems => 8.0,
            ResourceType::Silver => 5.0,
            ResourceType::Spices => 4.0,
            ResourceType::Copper => 2.0,
            ResourceType::Iron => 2.0,
            ResourceType::Furs => 3.0,
            ResourceType::Salt => 2.0,
            ResourceType::Coal => 1.5,
            ResourceType::Stone => 1.0,
            ResourceType::Timber => 1.0,
            ResourceType::Food => 1.0,
        }
    }

    /// Preferred terrain for this resource
    pub fn preferred_terrain(&self) -> &'static [&'static str] {
        match self {
            ResourceType::Iron | ResourceType::Coal | ResourceType::Copper => &["mountain", "hills"],
            ResourceType::Gold | ResourceType::Silver | ResourceType::Gems => &["mountain"],
            ResourceType::Stone => &["mountain", "hills", "badlands"],
            ResourceType::Timber => &["forest"],
            ResourceType::Food => &["grassland", "farmland"],
            ResourceType::Salt => &["desert", "coastal"],
            ResourceType::Spices => &["tropical", "forest"],
            ResourceType::Furs => &["tundra", "boreal"],
        }
    }
}

/// A resource site (mine, quarry, farm, etc.)
#[derive(Clone, Debug)]
pub struct ResourceSite {
    /// Location
    pub x: usize,
    pub y: usize,
    /// Type of resource
    pub resource: ResourceType,
    /// Whether the resource is depleted
    pub depleted: bool,
    /// Year discovered
    pub discovered: Year,
    /// Year depleted (if applicable)
    pub depleted_year: Option<Year>,
    /// Controlling faction (if any)
    pub faction: Option<FactionId>,
}

/// A trade route between two locations
#[derive(Clone, Debug)]
pub struct TradeRoute {
    /// Unique identifier
    pub id: TradeRouteId,
    /// Start location (settlement position)
    pub start: (usize, usize),
    /// End location
    pub end: (usize, usize),
    /// Path tiles
    pub path: Vec<(usize, usize)>,
    /// Whether route is active
    pub active: bool,
    /// Year established
    pub established: Year,
    /// Year abandoned (if applicable)
    pub abandoned: Option<Year>,
    /// Resources traded on this route
    pub resources: Vec<ResourceType>,
    /// Waypoints along the route (inns, waystations)
    pub waypoints: Vec<(usize, usize, WaypointType)>,
}

/// Type of waypoint along a trade route
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WaypointType {
    Inn,
    TradePost,
    Watchtower,
    Waystation,
    Bridge,
}

impl WaypointType {
    pub fn name(&self) -> &'static str {
        match self {
            WaypointType::Inn => "Inn",
            WaypointType::TradePost => "Trade Post",
            WaypointType::Watchtower => "Watchtower",
            WaypointType::Waystation => "Waystation",
            WaypointType::Bridge => "Bridge",
        }
    }
}

/// Registry of trade routes and resources
#[derive(Clone, Debug)]
pub struct TradeRegistry {
    /// All trade routes
    pub routes: HashMap<TradeRouteId, TradeRoute>,
    /// Resource sites
    pub resources: Vec<ResourceSite>,
    /// Resource sites by location
    pub resources_by_location: HashMap<(usize, usize), usize>,
    /// Next route ID
    next_id: u32,
}

impl Default for TradeRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl TradeRegistry {
    pub fn new() -> Self {
        Self {
            routes: HashMap::new(),
            resources: Vec::new(),
            resources_by_location: HashMap::new(),
            next_id: 0,
        }
    }

    /// Add a trade route
    pub fn add_route(&mut self, route: TradeRoute) {
        self.routes.insert(route.id, route);
    }

    /// Add a resource site
    pub fn add_resource(&mut self, site: ResourceSite) {
        let idx = self.resources.len();
        let loc = (site.x, site.y);
        self.resources_by_location.insert(loc, idx);
        self.resources.push(site);
    }

    /// Get resource at location
    pub fn resource_at(&self, x: usize, y: usize) -> Option<&ResourceSite> {
        self.resources_by_location.get(&(x, y))
            .map(|&idx| &self.resources[idx])
    }

    /// Generate new route ID
    pub fn new_id(&mut self) -> TradeRouteId {
        let id = TradeRouteId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Get active trade routes
    pub fn active_routes(&self) -> impl Iterator<Item = &TradeRoute> {
        self.routes.values().filter(|r| r.active)
    }

    /// Check if a tile is on any trade route
    pub fn is_on_route(&self, x: usize, y: usize) -> bool {
        self.routes.values().any(|r| r.path.contains(&(x, y)))
    }
}

/// Generate trade network for the world
pub fn generate_trade_network(
    territories: &TerritoryRegistry,
    heightmap: &Tilemap<f32>,
    water_bodies: &Tilemap<WaterBodyId>,
    biomes: &Tilemap<ExtendedBiome>,
    seed: u64,
) -> TradeRegistry {
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0x7FADE));
    let mut registry = TradeRegistry::new();

    let width = heightmap.width;
    let height = heightmap.height;

    // Generate resource sites
    generate_resource_sites(
        &mut registry,
        heightmap,
        biomes,
        territories,
        width,
        height,
        &mut rng,
    );

    // Generate trade routes between settlements
    generate_trade_routes(
        &mut registry,
        territories,
        heightmap,
        water_bodies,
        width,
        height,
        &mut rng,
    );

    println!("  Generated {} resource sites and {} trade routes",
        registry.resources.len(),
        registry.routes.len()
    );

    registry
}

/// Generate resource sites
fn generate_resource_sites(
    registry: &mut TradeRegistry,
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    territories: &TerritoryRegistry,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) {
    // Scale resources with map size
    let map_area = width * height;
    let scale = (map_area as f32 / (512.0 * 256.0)).sqrt();
    let num_resources = ((20.0 * scale) as usize).clamp(10, 50);

    let mut used: HashSet<(usize, usize)> = HashSet::new();

    for _ in 0..num_resources {
        // Pick a resource type
        let resource = pick_resource_type(rng);

        // Find suitable location
        let location = find_resource_location(
            resource,
            heightmap,
            biomes,
            &used,
            width,
            height,
            rng,
        );

        if let Some((x, y)) = location {
            // Mark area as used
            for dy in 0..5 {
                for dx in 0..5 {
                    used.insert((x.wrapping_add(dx), y.wrapping_add(dy)));
                    used.insert((x.wrapping_sub(dx), y.wrapping_add(dy)));
                    used.insert((x.wrapping_add(dx), y.wrapping_sub(dy)));
                    used.insert((x.wrapping_sub(dx), y.wrapping_sub(dy)));
                }
            }

            // Determine faction ownership
            let faction = territories.faction_at(x, y);

            // Determine if depleted
            let depleted = rng.gen_bool(0.3);
            let discovered = Year::years_ago(rng.gen_range(100..1000));
            let depleted_year = if depleted {
                Some(Year::years_ago(rng.gen_range(20..discovered.age())))
            } else {
                None
            };

            registry.add_resource(ResourceSite {
                x,
                y,
                resource,
                depleted,
                discovered,
                depleted_year,
                faction,
            });
        }
    }
}

/// Pick a resource type
fn pick_resource_type(rng: &mut ChaCha8Rng) -> ResourceType {
    let resources = ResourceType::all();
    let weights: Vec<u32> = resources.iter().map(|r| {
        match r {
            ResourceType::Gold | ResourceType::Gems => 5,
            ResourceType::Silver | ResourceType::Spices => 10,
            ResourceType::Iron | ResourceType::Copper | ResourceType::Coal => 20,
            ResourceType::Timber | ResourceType::Food => 25,
            _ => 15,
        }
    }).collect();

    let total: u32 = weights.iter().sum();
    let mut r = rng.gen_range(0..total);

    for (i, &weight) in weights.iter().enumerate() {
        if r < weight {
            return resources[i];
        }
        r -= weight;
    }

    ResourceType::Iron
}

/// Find a suitable location for a resource
fn find_resource_location(
    resource: ResourceType,
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    used: &HashSet<(usize, usize)>,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) -> Option<(usize, usize)> {
    let preferred = resource.preferred_terrain();
    let mut candidates: Vec<(usize, usize, f32)> = Vec::new();

    for y in 5..(height - 5) {
        for x in 5..(width - 5) {
            if used.contains(&(x, y)) {
                continue;
            }

            let elev = *heightmap.get(x, y);
            if elev < 0.0 {
                continue;
            }

            let biome = *biomes.get(x, y);
            let terrain = categorize_terrain(biome, elev);

            if !preferred.iter().any(|&p| terrain.contains(p)) {
                continue;
            }

            candidates.push((x, y, rng.gen::<f32>()));
        }
    }

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    Some((candidates[0].0, candidates[0].1))
}

/// Categorize terrain for resource placement
fn categorize_terrain(biome: ExtendedBiome, elev: f32) -> &'static str {
    if elev > 1500.0 {
        return "mountain";
    }
    if elev > 800.0 {
        return "hills";
    }

    match biome {
        ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest |
        ExtendedBiome::TemperateRainforest => "forest",

        ExtendedBiome::TropicalForest | ExtendedBiome::TropicalRainforest => "tropical",

        ExtendedBiome::TemperateGrassland | ExtendedBiome::Foothills => "grassland",

        ExtendedBiome::Savanna => "savanna",

        ExtendedBiome::Desert | ExtendedBiome::SingingDunes => "desert",

        ExtendedBiome::Tundra | ExtendedBiome::AuroraWastes => "tundra",

        ExtendedBiome::Lagoon | ExtendedBiome::CoastalWater => "coastal",

        ExtendedBiome::Ashlands => "wasteland",

        _ => "other",
    }
}

/// Generate trade routes between settlements
fn generate_trade_routes(
    registry: &mut TradeRegistry,
    territories: &TerritoryRegistry,
    heightmap: &Tilemap<f32>,
    water_bodies: &Tilemap<WaterBodyId>,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) {
    // Get settlements that can trade (cities, towns, capitals)
    let tradeable: Vec<&Settlement> = territories.settlements.values()
        .filter(|s| matches!(s.settlement_type,
            SettlementType::Capital | SettlementType::City |
            SettlementType::Town | SettlementType::Outpost
        ))
        .collect();

    if tradeable.len() < 2 {
        return;
    }

    // Generate routes between nearby settlements
    for i in 0..tradeable.len() {
        for j in (i + 1)..tradeable.len() {
            let s1 = tradeable[i];
            let s2 = tradeable[j];

            // Calculate distance
            let dx = (s1.x as i32 - s2.x as i32).abs();
            let dy = (s1.y as i32 - s2.y as i32).abs();
            let dist = ((dx * dx + dy * dy) as f32).sqrt();

            // Only create routes for nearby settlements
            let max_dist = (width as f32 * 0.4).max(50.0);
            if dist > max_dist {
                continue;
            }

            // Chance to create route based on distance
            let chance = 1.0 - (dist / max_dist);
            if !rng.gen_bool(chance as f64 * 0.5) {
                continue;
            }

            // Find path using A*
            let path = find_path(
                (s1.x, s1.y),
                (s2.x, s2.y),
                heightmap,
                water_bodies,
                width,
                height,
            );

            if path.is_empty() {
                continue;
            }

            // Generate waypoints
            let waypoints = generate_waypoints(&path, heightmap, water_bodies, rng);

            // Determine if route is active
            let active = s1.is_active() && s2.is_active();

            // Pick resources traded
            let resources: Vec<ResourceType> = (0..rng.gen_range(1..=3))
                .map(|_| *pick_random(rng, ResourceType::all()))
                .collect();

            let id = registry.new_id();
            let route = TradeRoute {
                id,
                start: (s1.x, s1.y),
                end: (s2.x, s2.y),
                path,
                active,
                established: Year::years_ago(rng.gen_range(50..500)),
                abandoned: if !active { Some(Year::years_ago(rng.gen_range(10..200))) } else { None },
                resources,
                waypoints,
            };

            registry.add_route(route);
        }
    }
}

/// A* pathfinding for trade routes
fn find_path(
    start: (usize, usize),
    end: (usize, usize),
    heightmap: &Tilemap<f32>,
    water_bodies: &Tilemap<WaterBodyId>,
    width: usize,
    height: usize,
) -> Vec<(usize, usize)> {
    #[derive(Clone, Eq, PartialEq)]
    struct Node {
        pos: (usize, usize),
        g: i32,
        f: i32,
    }

    impl Ord for Node {
        fn cmp(&self, other: &Self) -> Ordering {
            other.f.cmp(&self.f)
        }
    }

    impl PartialOrd for Node {
        fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
            Some(self.cmp(other))
        }
    }

    let mut open = BinaryHeap::new();
    let mut came_from: HashMap<(usize, usize), (usize, usize)> = HashMap::new();
    let mut g_score: HashMap<(usize, usize), i32> = HashMap::new();

    let heuristic = |pos: (usize, usize)| -> i32 {
        let dx = (pos.0 as i32 - end.0 as i32).abs();
        let dy = (pos.1 as i32 - end.1 as i32).abs();
        dx + dy
    };

    g_score.insert(start, 0);
    open.push(Node { pos: start, g: 0, f: heuristic(start) });

    while let Some(current) = open.pop() {
        if current.pos == end {
            // Reconstruct path
            let mut path = vec![end];
            let mut pos = end;
            while let Some(&prev) = came_from.get(&pos) {
                path.push(prev);
                pos = prev;
            }
            path.reverse();
            return path;
        }

        // Check neighbors
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1), (-1, -1), (-1, 1), (1, -1), (1, 1)] {
            let nx = (current.pos.0 as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (current.pos.1 as i32 + dy).clamp(0, height as i32 - 1) as usize;
            let neighbor = (nx, ny);

            // Calculate movement cost
            let elev = *heightmap.get(nx, ny);
            let water = *water_bodies.get(nx, ny);

            // Skip deep water
            if elev < -50.0 {
                continue;
            }

            // Higher cost for water, mountains
            let terrain_cost = if water != WaterBodyId::NONE {
                5 // Bridge needed
            } else if elev > 1500.0 {
                4 // Mountain
            } else if elev > 800.0 {
                2 // Hills
            } else {
                1 // Flat
            };

            let diagonal = if dx != 0 && dy != 0 { 14 } else { 10 };
            let cost = terrain_cost * diagonal / 10;

            let tentative_g = current.g + cost;
            let current_g = g_score.get(&neighbor).copied().unwrap_or(i32::MAX);

            if tentative_g < current_g {
                came_from.insert(neighbor, current.pos);
                g_score.insert(neighbor, tentative_g);
                let f = tentative_g + heuristic(neighbor);
                open.push(Node { pos: neighbor, g: tentative_g, f });
            }
        }

        // Limit search
        if g_score.len() > 5000 {
            break;
        }
    }

    Vec::new() // No path found
}

/// Generate waypoints along a trade route
fn generate_waypoints(
    path: &[(usize, usize)],
    heightmap: &Tilemap<f32>,
    water_bodies: &Tilemap<WaterBodyId>,
    rng: &mut ChaCha8Rng,
) -> Vec<(usize, usize, WaypointType)> {
    let mut waypoints = Vec::new();

    if path.len() < 10 {
        return waypoints;
    }

    // Place waypoints every 15-25 tiles
    let spacing = rng.gen_range(15..25);
    let mut last_waypoint = 0;

    for (i, &(x, y)) in path.iter().enumerate() {
        if i < spacing || i - last_waypoint < spacing {
            continue;
        }

        let water = *water_bodies.get(x, y);
        let elev = *heightmap.get(x, y);

        // Determine waypoint type
        let waypoint_type = if water != WaterBodyId::NONE {
            WaypointType::Bridge
        } else if elev > 1000.0 {
            WaypointType::Watchtower
        } else if rng.gen_bool(0.3) {
            WaypointType::Inn
        } else if rng.gen_bool(0.3) {
            WaypointType::TradePost
        } else {
            WaypointType::Waystation
        };

        waypoints.push((x, y, waypoint_type));
        last_waypoint = i;
    }

    waypoints
}

/// Helper to pick a random element
fn pick_random<'a, T>(rng: &mut ChaCha8Rng, items: &'a [T]) -> &'a T {
    &items[rng.gen_range(0..items.len())]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::factions::generate_factions;
    use crate::history::territories::generate_territories;

    #[test]
    fn test_trade_generation() {
        let heightmap = Tilemap::new_with(64, 32, 100.0f32);
        let biomes = Tilemap::new_with(64, 32, ExtendedBiome::TemperateGrassland);
        let water_bodies = Tilemap::new_with(64, 32, WaterBodyId::NONE);

        let factions = generate_factions(&heightmap, &biomes, 42);
        let territories = generate_territories(&factions, &heightmap, &biomes, &water_bodies, 42);
        let trade = generate_trade_network(&territories, &heightmap, &water_bodies, &biomes, 42);

        println!("Resources: {}", trade.resources.len());
        println!("Routes: {}", trade.routes.len());
    }
}
