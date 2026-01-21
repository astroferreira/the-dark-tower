//! Territory and settlement generation
//!
//! Places settlements and defines faction territories based on terrain preferences.

use std::collections::{HashMap, HashSet, VecDeque};

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;
use crate::water_bodies::WaterBodyId;

use super::factions::{Faction, FactionRegistry};
use super::naming::NameGenerator;
use super::types::*;

/// A faction's territory (claimed area of the map)
#[derive(Clone, Debug)]
pub struct Territory {
    /// Faction that claims this territory
    pub faction: FactionId,
    /// Tiles claimed by this faction
    pub tiles: HashSet<(usize, usize)>,
    /// Approximate center of the territory
    pub center: (usize, usize),
    /// Year territory was established
    pub established: Year,
    /// Year territory was lost (if applicable)
    pub lost: Option<Year>,
}

impl Territory {
    /// Get the number of tiles in this territory
    pub fn size(&self) -> usize {
        self.tiles.len()
    }

    /// Check if a tile is in this territory
    pub fn contains(&self, x: usize, y: usize) -> bool {
        self.tiles.contains(&(x, y))
    }
}

/// A settlement (city, town, village, etc.)
#[derive(Clone, Debug)]
pub struct Settlement {
    /// Unique identifier
    pub id: SettlementId,
    /// Name of the settlement
    pub name: String,
    /// Type of settlement
    pub settlement_type: SettlementType,
    /// Faction that built this settlement
    pub original_faction: FactionId,
    /// Current owning faction (may differ from original)
    pub current_faction: Option<FactionId>,
    /// Location on the map
    pub x: usize,
    pub y: usize,
    /// Size in tiles
    pub size: usize,
    /// Current state
    pub state: SettlementState,
    /// Year founded
    pub founded: Year,
    /// Year abandoned/destroyed (if applicable)
    pub abandoned: Option<Year>,
    /// Reason for abandonment
    pub abandonment_reason: Option<AbandonmentReason>,
    /// Peak population
    pub peak_population: u32,
    /// Architecture style (from founding faction)
    pub architecture: ArchitectureStyle,
    /// History of occupations (faction, start year, end year)
    pub occupations: Vec<(FactionId, Year, Option<Year>)>,
}

impl Settlement {
    /// Check if this settlement is active (not abandoned/ruined/destroyed)
    pub fn is_active(&self) -> bool {
        matches!(self.state, SettlementState::Thriving | SettlementState::Declining)
    }

    /// Get the age of this settlement
    pub fn age(&self) -> i32 {
        if let Some(abandoned) = self.abandoned {
            abandoned.0 - self.founded.0
        } else {
            -self.founded.0
        }
    }
}

/// Registry of all territories and settlements
#[derive(Clone)]
pub struct TerritoryRegistry {
    /// All territories
    pub territories: Vec<Territory>,
    /// All settlements by ID
    pub settlements: HashMap<SettlementId, Settlement>,
    /// Settlements indexed by location
    pub settlements_by_location: HashMap<(usize, usize), SettlementId>,
    /// Territory map (which faction controls each tile)
    pub territory_map: Tilemap<Option<FactionId>>,
    /// Next settlement ID
    next_settlement_id: u32,
}

impl TerritoryRegistry {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            territories: Vec::new(),
            settlements: HashMap::new(),
            settlements_by_location: HashMap::new(),
            territory_map: Tilemap::new_with(width, height, None),
            next_settlement_id: 0,
        }
    }

    /// Add a settlement
    pub fn add_settlement(&mut self, settlement: Settlement) {
        let id = settlement.id;
        let loc = (settlement.x, settlement.y);
        self.settlements_by_location.insert(loc, id);
        self.settlements.insert(id, settlement);
    }

    /// Get settlement at a location
    pub fn settlement_at(&self, x: usize, y: usize) -> Option<&Settlement> {
        self.settlements_by_location.get(&(x, y))
            .and_then(|id| self.settlements.get(id))
    }

    /// Get faction controlling a tile
    pub fn faction_at(&self, x: usize, y: usize) -> Option<FactionId> {
        *self.territory_map.get(x, y)
    }

    /// Generate a new settlement ID
    pub fn new_settlement_id(&mut self) -> SettlementId {
        let id = SettlementId(self.next_settlement_id);
        self.next_settlement_id += 1;
        id
    }

    /// Get all settlements for a faction
    pub fn settlements_for_faction(&self, faction: FactionId) -> Vec<&Settlement> {
        self.settlements.values()
            .filter(|s| s.original_faction == faction)
            .collect()
    }
}

/// Generate territories and settlements for all factions
pub fn generate_territories(
    factions: &FactionRegistry,
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    water_bodies: &Tilemap<WaterBodyId>,
    seed: u64,
) -> TerritoryRegistry {
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0x7E6610AE));
    let name_gen = NameGenerator::new(seed);
    let width = heightmap.width;
    let height = heightmap.height;

    let mut registry = TerritoryRegistry::new(width, height);

    // Compute terrain desirability for each tile
    let desirability = compute_terrain_desirability(heightmap, biomes, water_bodies);

    // Place capital for each faction
    let mut used_locations: HashSet<(usize, usize)> = HashSet::new();

    for faction in factions.all() {
        // Find best location for this faction's capital
        let capital_loc = find_best_location(
            faction,
            &desirability,
            &biomes,
            &used_locations,
            width,
            height,
            &mut rng,
        );

        if let Some((cx, cy)) = capital_loc {
            // Mark area as used
            mark_area_used(&mut used_locations, cx, cy, 30, width, height);

            // Create capital settlement
            let capital = create_settlement(
                &mut registry,
                faction,
                SettlementType::Capital,
                cx,
                cy,
                &name_gen,
                &mut rng,
            );

            // Generate territory around capital using flood fill
            let territory = generate_territory(
                faction,
                cx,
                cy,
                &desirability,
                heightmap,
                water_bodies,
                &registry.territory_map,
                width,
                height,
                &mut rng,
            );

            // Apply territory to map
            for &(tx, ty) in &territory.tiles {
                registry.territory_map.set(tx, ty, Some(faction.id));
            }

            registry.territories.push(territory);
        }
    }

    // Generate additional settlements for each faction
    for faction in factions.all() {
        let num_settlements = (faction.peak_settlements as usize).saturating_sub(1); // -1 for capital

        for _ in 0..num_settlements {
            // Find location within territory
            let loc = find_settlement_location(
                faction.id,
                &registry.territory_map,
                &desirability,
                &used_locations,
                width,
                height,
                &mut rng,
            );

            if let Some((x, y)) = loc {
                mark_area_used(&mut used_locations, x, y, 15, width, height);

                // Pick settlement type based on terrain and faction
                let settlement_type = pick_settlement_type(
                    faction,
                    x,
                    y,
                    biomes,
                    &mut rng,
                );

                create_settlement(
                    &mut registry,
                    faction,
                    settlement_type,
                    x,
                    y,
                    &name_gen,
                    &mut rng,
                );
            }
        }
    }

    println!("  Generated {} territories with {} settlements",
        registry.territories.len(),
        registry.settlements.len()
    );

    registry
}

/// Compute terrain desirability for settlements
fn compute_terrain_desirability(
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    water_bodies: &Tilemap<WaterBodyId>,
) -> Tilemap<f32> {
    let width = heightmap.width;
    let height = heightmap.height;
    let mut desirability = Tilemap::new_with(width, height, 0.0f32);

    for y in 0..height {
        for x in 0..width {
            let elev = *heightmap.get(x, y);
            let biome = *biomes.get(x, y);
            let water = *water_bodies.get(x, y);

            // Base desirability from biome
            let mut score = biome_desirability(biome);

            // Prefer land over water
            if elev < 0.0 {
                score = 0.0;
            }

            // Bonus for being near water (but not in it)
            if water == WaterBodyId::NONE {
                let near_water = is_near_water(x, y, water_bodies, 5);
                if near_water {
                    score += 0.2;
                }
            }

            // Penalty for extreme elevations
            if elev > 2000.0 {
                score *= 0.5;
            } else if elev > 1000.0 {
                score *= 0.8;
            }

            // Prefer flat areas (low slope)
            // (simplified - just check elevation variance in nearby tiles)
            let slope = compute_local_slope(x, y, heightmap);
            if slope > 200.0 {
                score *= 0.6;
            } else if slope > 100.0 {
                score *= 0.8;
            }

            desirability.set(x, y, score);
        }
    }

    desirability
}

/// Get base desirability for a biome
fn biome_desirability(biome: ExtendedBiome) -> f32 {
    match biome {
        // High desirability - fertile, temperate lands
        ExtendedBiome::TemperateGrassland => 1.0,
        ExtendedBiome::TemperateForest => 0.9,
        ExtendedBiome::TemperateRainforest => 0.8,
        ExtendedBiome::Foothills => 0.8,

        // Medium-high - livable but challenging
        ExtendedBiome::BorealForest => 0.7,
        ExtendedBiome::Savanna => 0.7,
        ExtendedBiome::TropicalForest => 0.6,
        ExtendedBiome::TropicalRainforest => 0.5,

        // Medium - marginal lands
        ExtendedBiome::Desert => 0.3,
        ExtendedBiome::Tundra => 0.3,
        ExtendedBiome::AlpineTundra => 0.4,
        ExtendedBiome::Marsh => 0.4,
        ExtendedBiome::Bog => 0.4,

        // Low (but still possible)
        ExtendedBiome::Swamp => 0.2,
        ExtendedBiome::VolcanicWasteland => 0.1,
        ExtendedBiome::Ice => 0.05,
        ExtendedBiome::SnowyPeaks => 0.1,

        // Water - not suitable
        ExtendedBiome::DeepOcean | ExtendedBiome::Ocean | ExtendedBiome::CoastalWater => 0.0,

        // Default for other biomes
        _ => 0.5,
    }
}

/// Check if a tile is near water
fn is_near_water(x: usize, y: usize, water_bodies: &Tilemap<WaterBodyId>, radius: usize) -> bool {
    let width = water_bodies.width;
    let height = water_bodies.height;

    for dy in 0..=radius {
        for dx in 0..=radius {
            if dx == 0 && dy == 0 {
                continue;
            }

            for (sx, sy) in [
                (x.wrapping_add(dx), y.wrapping_add(dy)),
                (x.wrapping_sub(dx), y.wrapping_add(dy)),
                (x.wrapping_add(dx), y.wrapping_sub(dy)),
                (x.wrapping_sub(dx), y.wrapping_sub(dy)),
            ] {
                if sx < width && sy < height {
                    if *water_bodies.get(sx, sy) != WaterBodyId::NONE {
                        return true;
                    }
                }
            }
        }
    }

    false
}

/// Compute local slope (elevation variance)
fn compute_local_slope(x: usize, y: usize, heightmap: &Tilemap<f32>) -> f32 {
    let width = heightmap.width;
    let height = heightmap.height;
    let center = *heightmap.get(x, y);

    let mut max_diff = 0.0f32;

    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }

            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

            let neighbor = *heightmap.get(nx, ny);
            max_diff = max_diff.max((center - neighbor).abs());
        }
    }

    max_diff
}

/// Find the best location for a faction's capital
fn find_best_location(
    faction: &Faction,
    desirability: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    used: &HashSet<(usize, usize)>,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) -> Option<(usize, usize)> {
    let terrain_prefs = faction.species.preferred_terrain();

    // Collect candidate locations
    let mut candidates: Vec<(usize, usize, f32)> = Vec::new();

    for y in 5..(height - 5) {
        for x in 5..(width - 5) {
            if used.contains(&(x, y)) {
                continue;
            }

            let base_score = *desirability.get(x, y);
            if base_score < 0.1 {
                continue;
            }

            // Bonus for matching terrain preference
            let biome = *biomes.get(x, y);
            let terrain_bonus = if terrain_matches_preference(biome, terrain_prefs) {
                0.5
            } else {
                0.0
            };

            let score = base_score + terrain_bonus;
            candidates.push((x, y, score));
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // Sort by score and pick from top candidates with some randomness
    candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    let top_n = candidates.len().min(20);
    let idx = rng.gen_range(0..top_n);

    Some((candidates[idx].0, candidates[idx].1))
}

/// Check if a biome matches terrain preferences
fn terrain_matches_preference(biome: ExtendedBiome, prefs: &[TerrainPreference]) -> bool {
    for pref in prefs {
        let matches = match pref {
            TerrainPreference::Mountain => matches!(biome,
                ExtendedBiome::AlpineTundra | ExtendedBiome::SnowyPeaks |
                ExtendedBiome::RazorPeaks
            ),
            TerrainPreference::Forest => matches!(biome,
                ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest |
                ExtendedBiome::TropicalForest | ExtendedBiome::TropicalRainforest |
                ExtendedBiome::TemperateRainforest
            ),
            TerrainPreference::Plains => matches!(biome,
                ExtendedBiome::TemperateGrassland | ExtendedBiome::Foothills |
                ExtendedBiome::Savanna
            ),
            TerrainPreference::Desert => matches!(biome,
                ExtendedBiome::Desert | ExtendedBiome::SingingDunes |
                ExtendedBiome::SaltFlats
            ),
            TerrainPreference::Swamp => matches!(biome,
                ExtendedBiome::Swamp | ExtendedBiome::MangroveSaltmarsh |
                ExtendedBiome::Marsh
            ),
            TerrainPreference::Tundra => matches!(biome,
                ExtendedBiome::Tundra | ExtendedBiome::Ice |
                ExtendedBiome::AuroraWastes
            ),
            TerrainPreference::Coastal => matches!(biome,
                ExtendedBiome::Lagoon | ExtendedBiome::CoastalWater |
                ExtendedBiome::CoralReef
            ),
            TerrainPreference::Underground => false, // Surface biomes don't match
            TerrainPreference::Volcanic => matches!(biome,
                ExtendedBiome::VolcanicWasteland | ExtendedBiome::LavaLake
            ),
            TerrainPreference::Hills => matches!(biome,
                ExtendedBiome::Foothills | ExtendedBiome::Bog
            ),
            TerrainPreference::Temperate => matches!(biome,
                ExtendedBiome::TemperateGrassland | ExtendedBiome::TemperateForest |
                ExtendedBiome::Savanna
            ),
            TerrainPreference::Wasteland => matches!(biome,
                ExtendedBiome::Ashlands | ExtendedBiome::VolcanicWasteland |
                ExtendedBiome::SaltFlats
            ),
        };

        if matches {
            return true;
        }
    }

    false
}

/// Mark an area as used (preventing overlap)
fn mark_area_used(
    used: &mut HashSet<(usize, usize)>,
    cx: usize,
    cy: usize,
    radius: usize,
    width: usize,
    height: usize,
) {
    let r = radius as i32;
    for dy in -r..=r {
        for dx in -r..=r {
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= r * r {
                let nx = (cx as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (cy as i32 + dy).clamp(0, height as i32 - 1) as usize;
                used.insert((nx, ny));
            }
        }
    }
}

/// Generate territory around a capital using flood fill
fn generate_territory(
    faction: &Faction,
    cx: usize,
    cy: usize,
    desirability: &Tilemap<f32>,
    heightmap: &Tilemap<f32>,
    water_bodies: &Tilemap<WaterBodyId>,
    existing: &Tilemap<Option<FactionId>>,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) -> Territory {
    let mut tiles = HashSet::new();
    let mut queue = VecDeque::new();

    // Target territory size based on faction
    let target_size = match faction.culture {
        CultureType::Expansionist => rng.gen_range(800..1500),
        CultureType::Isolationist => rng.gen_range(100..300),
        CultureType::Nomadic => rng.gen_range(50..150),
        _ => rng.gen_range(300..700),
    };

    queue.push_back((cx, cy));
    tiles.insert((cx, cy));

    while let Some((x, y)) = queue.pop_front() {
        if tiles.len() >= target_size {
            break;
        }

        // Check neighbors
        for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
            let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

            if tiles.contains(&(nx, ny)) {
                continue;
            }

            // Skip if already claimed by another faction
            if existing.get(nx, ny).is_some() {
                continue;
            }

            // Skip water
            if *heightmap.get(nx, ny) < 0.0 {
                continue;
            }

            // Probability to expand based on desirability
            let score = *desirability.get(nx, ny);
            if score < 0.1 {
                continue;
            }

            if rng.gen_bool((score * 0.8) as f64) {
                tiles.insert((nx, ny));
                queue.push_back((nx, ny));
            }
        }
    }

    Territory {
        faction: faction.id,
        tiles,
        center: (cx, cy),
        established: faction.founded,
        lost: faction.collapsed,
    }
}

/// Find a location for a new settlement within territory
fn find_settlement_location(
    faction_id: FactionId,
    territory_map: &Tilemap<Option<FactionId>>,
    desirability: &Tilemap<f32>,
    used: &HashSet<(usize, usize)>,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) -> Option<(usize, usize)> {
    let mut candidates: Vec<(usize, usize, f32)> = Vec::new();

    for y in 5..(height - 5) {
        for x in 5..(width - 5) {
            // Must be in faction's territory
            if *territory_map.get(x, y) != Some(faction_id) {
                continue;
            }

            // Not already used
            if used.contains(&(x, y)) {
                continue;
            }

            let score = *desirability.get(x, y);
            if score > 0.2 {
                candidates.push((x, y, score));
            }
        }
    }

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    let top_n = candidates.len().min(10);
    let idx = rng.gen_range(0..top_n);

    Some((candidates[idx].0, candidates[idx].1))
}

/// Pick a settlement type based on faction and terrain
fn pick_settlement_type(
    faction: &Faction,
    x: usize,
    y: usize,
    biomes: &Tilemap<ExtendedBiome>,
    rng: &mut ChaCha8Rng,
) -> SettlementType {
    let biome = *biomes.get(x, y);

    // Culture-based preferences
    let weights: Vec<(SettlementType, u32)> = match faction.culture {
        CultureType::Militaristic => vec![
            (SettlementType::Fortress, 30),
            (SettlementType::City, 25),
            (SettlementType::Town, 20),
            (SettlementType::Village, 15),
            (SettlementType::Outpost, 10),
        ],
        CultureType::Religious => vec![
            (SettlementType::Temple, 30),
            (SettlementType::City, 25),
            (SettlementType::Town, 20),
            (SettlementType::Village, 25),
        ],
        CultureType::Industrial => vec![
            (SettlementType::Mine, 30),
            (SettlementType::City, 25),
            (SettlementType::Town, 25),
            (SettlementType::Village, 20),
        ],
        CultureType::Mercantile => vec![
            (SettlementType::City, 30),
            (SettlementType::Town, 30),
            (SettlementType::Outpost, 25),
            (SettlementType::Village, 15),
        ],
        CultureType::Nomadic => vec![
            (SettlementType::Outpost, 40),
            (SettlementType::Village, 40),
            (SettlementType::Town, 20),
        ],
        _ => vec![
            (SettlementType::City, 20),
            (SettlementType::Town, 30),
            (SettlementType::Village, 40),
            (SettlementType::Outpost, 10),
        ],
    };

    let total: u32 = weights.iter().map(|(_, w)| w).sum();
    let mut r = rng.gen_range(0..total);

    for (settlement_type, weight) in weights {
        if r < weight {
            return settlement_type;
        }
        r -= weight;
    }

    SettlementType::Village
}

/// Create a settlement
fn create_settlement(
    registry: &mut TerritoryRegistry,
    faction: &Faction,
    settlement_type: SettlementType,
    x: usize,
    y: usize,
    name_gen: &NameGenerator,
    rng: &mut ChaCha8Rng,
) -> SettlementId {
    let id = registry.new_settlement_id();
    let name = name_gen.settlement_name(faction.species, rng);

    // Determine size
    let (min_size, max_size) = settlement_type.size_range();
    let size = rng.gen_range(min_size..=max_size);

    // Determine population
    let (min_pop, max_pop) = settlement_type.population_range();
    let peak_population = rng.gen_range(min_pop..=max_pop);

    // Determine state based on faction status
    let (state, abandoned, abandonment_reason) = if faction.is_collapsed() {
        // Settlement is in ruins/abandoned
        let years_since_collapse = faction.years_collapsed();
        let state = if years_since_collapse > 500 {
            SettlementState::Destroyed
        } else if years_since_collapse > 200 {
            SettlementState::Ruined
        } else {
            SettlementState::Abandoned
        };

        (state, faction.collapsed, faction.collapse_reason)
    } else {
        // Active faction - some settlements may still decline
        if rng.gen_bool(0.1) {
            (SettlementState::Declining, None, None)
        } else {
            (SettlementState::Thriving, None, None)
        }
    };

    let settlement = Settlement {
        id,
        name,
        settlement_type,
        original_faction: faction.id,
        current_faction: if faction.is_collapsed() { None } else { Some(faction.id) },
        x,
        y,
        size,
        state,
        founded: faction.founded,
        abandoned,
        abandonment_reason,
        peak_population,
        architecture: faction.architecture,
        occupations: vec![(faction.id, faction.founded, faction.collapsed)],
    };

    registry.add_settlement(settlement);
    id
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::factions::generate_factions;

    #[test]
    fn test_territory_generation() {
        let heightmap = Tilemap::new_with(64, 32, 100.0f32);
        let biomes = Tilemap::new_with(64, 32, ExtendedBiome::TemperateGrassland);
        let water_bodies = Tilemap::new_with(64, 32, WaterBodyId::NONE);

        let factions = generate_factions(&heightmap, &biomes, 42);
        let territories = generate_territories(&factions, &heightmap, &biomes, &water_bodies, 42);

        assert!(!territories.territories.is_empty(), "Should have territories");
        assert!(!territories.settlements.is_empty(), "Should have settlements");

        for settlement in territories.settlements.values() {
            println!("{}: {} at ({}, {})",
                settlement.name,
                settlement.settlement_type.name(),
                settlement.x,
                settlement.y
            );
        }
    }
}
