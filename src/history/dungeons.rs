//! Dungeon and cave system generation
//!
//! Creates dungeon locations with historical significance that can contain artifacts.

use std::collections::HashMap;

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;

use super::types::*;
use super::naming::NameGenerator;
use super::territories::TerritoryRegistry;
use super::monsters::categorize_biome;

/// Origin type of a dungeon
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DungeonOrigin {
    AncientTomb,
    CollapsedMine,
    MonsterLair,
    FallenFortress,
    NaturalCave,
    AncientTemple,
    WizardsTower,
}

impl DungeonOrigin {
    pub fn all() -> &'static [DungeonOrigin] {
        &[
            DungeonOrigin::AncientTomb,
            DungeonOrigin::CollapsedMine,
            DungeonOrigin::MonsterLair,
            DungeonOrigin::FallenFortress,
            DungeonOrigin::NaturalCave,
            DungeonOrigin::AncientTemple,
            DungeonOrigin::WizardsTower,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            DungeonOrigin::AncientTomb => "Ancient Tomb",
            DungeonOrigin::CollapsedMine => "Collapsed Mine",
            DungeonOrigin::MonsterLair => "Monster Lair",
            DungeonOrigin::FallenFortress => "Fallen Fortress",
            DungeonOrigin::NaturalCave => "Natural Cave",
            DungeonOrigin::AncientTemple => "Ancient Temple",
            DungeonOrigin::WizardsTower => "Wizard's Tower",
        }
    }

    /// Get the origin type string for naming
    pub fn origin_type(&self) -> &'static str {
        match self {
            DungeonOrigin::AncientTomb => "tomb",
            DungeonOrigin::CollapsedMine => "mine",
            DungeonOrigin::MonsterLair => "cave",
            DungeonOrigin::FallenFortress => "fortress",
            DungeonOrigin::NaturalCave => "cave",
            DungeonOrigin::AncientTemple => "temple",
            DungeonOrigin::WizardsTower => "fortress",
        }
    }

    /// How many artifacts this dungeon type typically has
    pub fn artifact_capacity(&self) -> (usize, usize) {
        match self {
            DungeonOrigin::AncientTomb => (2, 5),
            DungeonOrigin::CollapsedMine => (0, 2),
            DungeonOrigin::MonsterLair => (1, 4),
            DungeonOrigin::FallenFortress => (2, 6),
            DungeonOrigin::NaturalCave => (0, 2),
            DungeonOrigin::AncientTemple => (3, 7),
            DungeonOrigin::WizardsTower => (2, 5),
        }
    }

    /// Weight for random selection
    pub fn weight(&self) -> u32 {
        match self {
            DungeonOrigin::AncientTomb => 20,
            DungeonOrigin::CollapsedMine => 15,
            DungeonOrigin::MonsterLair => 25,
            DungeonOrigin::FallenFortress => 15,
            DungeonOrigin::NaturalCave => 30,
            DungeonOrigin::AncientTemple => 10,
            DungeonOrigin::WizardsTower => 5,
        }
    }
}

/// A dungeon or significant cave system
#[derive(Clone, Debug)]
pub struct Dungeon {
    pub id: DungeonId,
    pub name: String,
    pub location: (usize, usize),
    pub depth_min: i32,
    pub depth_max: i32,
    pub original_purpose: DungeonOrigin,
    pub artifacts_present: Vec<ArtifactId>,
    pub history: Vec<String>,
    pub founded_year: Year,
    pub abandoned_year: Option<Year>,
    pub faction_origin: Option<FactionId>,
    /// Approximate size in tiles
    pub size: usize,
    /// Whether this dungeon has been explored
    pub explored: bool,
}

impl Dungeon {
    /// Get the full depth range as a string
    pub fn depth_range_str(&self) -> String {
        format!("z={} to z={}", self.depth_min, self.depth_max)
    }

    /// Get how old this dungeon is
    pub fn age(&self) -> i32 {
        self.founded_year.age()
    }
}

/// Registry of all dungeons
#[derive(Clone, Debug, Default)]
pub struct DungeonRegistry {
    pub dungeons: HashMap<DungeonId, Dungeon>,
    pub dungeons_by_location: HashMap<(usize, usize), DungeonId>,
    next_id: u32,
}

impl DungeonRegistry {
    pub fn new() -> Self {
        Self {
            dungeons: HashMap::new(),
            dungeons_by_location: HashMap::new(),
            next_id: 0,
        }
    }

    /// Add a dungeon to the registry
    pub fn add(&mut self, dungeon: Dungeon) {
        let id = dungeon.id;
        let loc = dungeon.location;
        self.dungeons_by_location.insert(loc, id);
        self.dungeons.insert(id, dungeon);
    }

    /// Get a dungeon by ID
    pub fn get(&self, id: DungeonId) -> Option<&Dungeon> {
        self.dungeons.get(&id)
    }

    /// Get a mutable reference to a dungeon by ID
    pub fn get_mut(&mut self, id: DungeonId) -> Option<&mut Dungeon> {
        self.dungeons.get_mut(&id)
    }

    /// Generate a new dungeon ID
    pub fn new_id(&mut self) -> DungeonId {
        let id = DungeonId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Get dungeon at location
    pub fn dungeon_at(&self, x: usize, y: usize) -> Option<&Dungeon> {
        self.dungeons_by_location.get(&(x, y))
            .and_then(|id| self.dungeons.get(id))
    }

    /// Get all dungeons
    pub fn all(&self) -> impl Iterator<Item = &Dungeon> {
        self.dungeons.values()
    }

    /// Get dungeons by origin type
    pub fn dungeons_by_origin(&self, origin: DungeonOrigin) -> Vec<&Dungeon> {
        self.dungeons.values()
            .filter(|d| d.original_purpose == origin)
            .collect()
    }
}

/// Generate dungeons for the world
pub fn generate_dungeons(
    territories: &TerritoryRegistry,
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    seed: u64,
) -> DungeonRegistry {
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0xD0BBEA57));
    let name_gen = NameGenerator::new(seed);
    let mut registry = DungeonRegistry::new();

    let width = heightmap.width;
    let height = heightmap.height;

    // Scale number of dungeons with map size
    let map_area = width * height;
    let base_dungeons = 15;
    let scale = (map_area as f32 / (512.0 * 256.0)).sqrt();
    let num_dungeons = ((base_dungeons as f32 * scale) as usize).clamp(8, 50);

    println!("  Generating {} dungeons...", num_dungeons);

    // Track used locations
    let mut used: Vec<(usize, usize)> = Vec::new();

    for _ in 0..num_dungeons {
        // Pick dungeon origin type
        let origin = pick_dungeon_origin(&mut rng);

        // Find suitable location
        let location = find_dungeon_location(
            origin,
            heightmap,
            biomes,
            &used,
            width,
            height,
            &mut rng,
        );

        if let Some((x, y)) = location {
            used.push((x, y));

            let id = registry.new_id();

            // Generate biome-aware name
            let biome = *biomes.get(x, y);
            let biome_category = categorize_biome(biome);
            let name = name_gen.dungeon_name_biome(origin.origin_type(), biome_category, &mut rng);

            // Determine depth range based on terrain
            let surface_height = *heightmap.get(x, y);
            let base_depth = if surface_height > 500.0 {
                rng.gen_range(-8..-2) // Mountains have deeper dungeons
            } else if surface_height > 0.0 {
                rng.gen_range(-6..-1) // Normal land
            } else {
                rng.gen_range(-4..0) // Near water
            };

            let depth_range = rng.gen_range(2..=6);
            let depth_min = base_depth - depth_range;
            let depth_max = base_depth;

            // Generate history
            let founded_years_ago = rng.gen_range(200..1500);
            let founded_year = Year::years_ago(founded_years_ago);

            let abandoned_year = if origin != DungeonOrigin::NaturalCave {
                let abandoned_years_ago = rng.gen_range(50..founded_years_ago);
                Some(Year::years_ago(abandoned_years_ago))
            } else {
                None
            };

            // Link to faction if near a settlement
            let faction_origin = find_nearest_faction(x, y, territories);

            // Generate history entries
            let history = generate_dungeon_history(origin, founded_year, abandoned_year, &name_gen, &mut rng);

            // Size based on origin
            let size = match origin {
                DungeonOrigin::AncientTomb => rng.gen_range(20..50),
                DungeonOrigin::CollapsedMine => rng.gen_range(30..80),
                DungeonOrigin::MonsterLair => rng.gen_range(15..40),
                DungeonOrigin::FallenFortress => rng.gen_range(50..120),
                DungeonOrigin::NaturalCave => rng.gen_range(40..100),
                DungeonOrigin::AncientTemple => rng.gen_range(40..80),
                DungeonOrigin::WizardsTower => rng.gen_range(20..50),
            };

            let dungeon = Dungeon {
                id,
                name,
                location: (x, y),
                depth_min,
                depth_max,
                original_purpose: origin,
                artifacts_present: Vec::new(), // Will be populated by artifact placement
                history,
                founded_year,
                abandoned_year,
                faction_origin,
                size,
                explored: false,
            };

            registry.add(dungeon);
        }
    }

    println!("    {} dungeons generated", registry.dungeons.len());
    registry
}

/// Pick a dungeon origin type
fn pick_dungeon_origin(rng: &mut ChaCha8Rng) -> DungeonOrigin {
    let origins = DungeonOrigin::all();
    let total_weight: u32 = origins.iter().map(|o| o.weight()).sum();
    let mut r = rng.gen_range(0..total_weight);

    for origin in origins {
        let weight = origin.weight();
        if r < weight {
            return *origin;
        }
        r -= weight;
    }

    DungeonOrigin::NaturalCave
}

/// Find a suitable location for a dungeon
fn find_dungeon_location(
    origin: DungeonOrigin,
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    used: &[(usize, usize)],
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) -> Option<(usize, usize)> {
    let min_distance = 20; // Minimum distance between dungeons

    let mut candidates: Vec<(usize, usize, f32)> = Vec::new();

    for y in 10..(height - 10) {
        for x in 10..(width - 10) {
            // Check distance from used locations
            let too_close = used.iter().any(|&(ux, uy)| {
                let dx = (x as i32 - ux as i32).abs();
                let dy = (y as i32 - uy as i32).abs();
                (dx * dx + dy * dy) < (min_distance * min_distance) as i32
            });

            if too_close {
                continue;
            }

            let elev = *heightmap.get(x, y);
            let biome = *biomes.get(x, y);

            // Skip water
            if elev < 0.0 {
                continue;
            }

            // Score based on origin type preferences
            let mut score = 0.5f32;

            match origin {
                DungeonOrigin::AncientTomb | DungeonOrigin::AncientTemple => {
                    // Prefer elevated, remote areas
                    if elev > 300.0 {
                        score += 0.3;
                    }
                }
                DungeonOrigin::CollapsedMine => {
                    // Prefer mountainous areas
                    if elev > 500.0 {
                        score += 0.4;
                    }
                }
                DungeonOrigin::MonsterLair | DungeonOrigin::NaturalCave => {
                    // Prefer wilderness areas (forests, mountains)
                    if matches!(biome,
                        ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest |
                        ExtendedBiome::AlpineTundra | ExtendedBiome::SnowyPeaks
                    ) {
                        score += 0.3;
                    }
                }
                DungeonOrigin::FallenFortress => {
                    // Prefer strategic locations (hills, passes)
                    if elev > 200.0 && elev < 800.0 {
                        score += 0.3;
                    }
                }
                DungeonOrigin::WizardsTower => {
                    // Prefer isolated, dramatic locations
                    if elev > 400.0 {
                        score += 0.4;
                    }
                }
            }

            candidates.push((x, y, score));
        }
    }

    if candidates.is_empty() {
        return None;
    }

    // Sort by score descending
    candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());

    // Pick from top candidates
    let top_n = candidates.len().min(20);
    let idx = rng.gen_range(0..top_n);

    Some((candidates[idx].0, candidates[idx].1))
}

/// Find nearest faction to a location
fn find_nearest_faction(
    x: usize,
    y: usize,
    territories: &TerritoryRegistry,
) -> Option<FactionId> {
    // Check if this location is in a territory
    territories.faction_at(x, y)
}

/// Generate history entries for a dungeon
fn generate_dungeon_history(
    origin: DungeonOrigin,
    founded: Year,
    abandoned: Option<Year>,
    name_gen: &NameGenerator,
    rng: &mut ChaCha8Rng,
) -> Vec<String> {
    let mut history = Vec::new();

    // Foundation event
    let founding_desc = match origin {
        DungeonOrigin::AncientTomb => "Royal burial chambers constructed",
        DungeonOrigin::CollapsedMine => "Mining operations begun",
        DungeonOrigin::MonsterLair => "Cave first inhabited by creatures",
        DungeonOrigin::FallenFortress => "Fortress construction completed",
        DungeonOrigin::NaturalCave => "Cave system discovered by explorers",
        DungeonOrigin::AncientTemple => "Temple consecrated to the old gods",
        DungeonOrigin::WizardsTower => "Wizard established arcane sanctum",
    };
    history.push(format!("{} years ago: {}", founded.age(), founding_desc));

    // Add intermediate events
    let num_events = rng.gen_range(1..=3);
    for _ in 0..num_events {
        let event_year = rng.gen_range(abandoned.map(|a| a.age()).unwrap_or(10)..founded.age());
        let event = match rng.gen_range(0..6) {
            0 => "Strange sounds reported from the depths",
            1 => "Expedition lost in the lower chambers",
            2 => "Treasure hunters explored the upper levels",
            3 => "Local villagers reported strange lights",
            4 => "Ancient wards began to fail",
            5 => "Mysterious disappearances in the area",
            _ => "Unknown event occurred",
        };
        history.push(format!("{} years ago: {}", event_year, event));
    }

    // Abandonment event
    if let Some(abandoned_year) = abandoned {
        let abandon_desc = match origin {
            DungeonOrigin::AncientTomb => "Tomb sealed after grave robbers struck",
            DungeonOrigin::CollapsedMine => "Mine collapsed, workers fled",
            DungeonOrigin::MonsterLair => "Creatures displaced, lair abandoned",
            DungeonOrigin::FallenFortress => "Fortress fell to enemy forces",
            DungeonOrigin::NaturalCave => "Cave system blocked by rockfall",
            DungeonOrigin::AncientTemple => "Temple abandoned as faith waned",
            DungeonOrigin::WizardsTower => "Wizard disappeared, tower sealed",
        };
        history.push(format!("{} years ago: {}", abandoned_year.age(), abandon_desc));
    }

    // Sort by age (oldest first)
    history.sort_by(|a, b| {
        let age_a: i32 = a.split(" years ago").next().unwrap_or("0").parse().unwrap_or(0);
        let age_b: i32 = b.split(" years ago").next().unwrap_or("0").parse().unwrap_or(0);
        age_b.cmp(&age_a)
    });

    history
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dungeon_generation() {
        let heightmap = Tilemap::new_with(128, 64, 200.0f32);
        let biomes = Tilemap::new_with(128, 64, ExtendedBiome::TemperateForest);
        let territories = TerritoryRegistry::new(128, 64);

        let registry = generate_dungeons(&territories, &heightmap, &biomes, 42);

        assert!(!registry.dungeons.is_empty(), "Should have generated dungeons");

        for dungeon in registry.all() {
            println!("{} ({:?})", dungeon.name, dungeon.original_purpose);
            println!("  Location: ({}, {}) | Depth: {}", dungeon.location.0, dungeon.location.1, dungeon.depth_range_str());
            for entry in &dungeon.history {
                println!("  {}", entry);
            }
        }
    }
}
