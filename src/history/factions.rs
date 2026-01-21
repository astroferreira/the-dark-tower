//! Faction generation and management
//!
//! Factions represent civilizations that once existed in the world.
//! Each faction has a species, culture, architecture style, and relationships with other factions.

use std::collections::HashMap;

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;

use super::naming::NameGenerator;
use super::types::*;
use super::monsters::BiomeCategory;

/// A faction (civilization) in the world
#[derive(Clone, Debug)]
pub struct Faction {
    /// Unique identifier
    pub id: FactionId,
    /// Name of the faction
    pub name: String,
    /// Species that makes up this faction
    pub species: Species,
    /// Cultural characteristics
    pub culture: CultureType,
    /// Architectural style for buildings
    pub architecture: ArchitectureStyle,
    /// Year the faction was founded (negative = years ago)
    pub founded: Year,
    /// Year the faction collapsed (None if still active)
    pub collapsed: Option<Year>,
    /// Reason for collapse (if collapsed)
    pub collapse_reason: Option<AbandonmentReason>,
    /// Primary color for faction (for map display)
    pub color: (u8, u8, u8),
    /// Capital settlement ID (if any)
    pub capital: Option<SettlementId>,
    /// Total number of settlements at peak
    pub peak_settlements: u32,
    /// Approximate peak population
    pub peak_population: u32,
}

impl Faction {
    /// Check if this faction has collapsed
    pub fn is_collapsed(&self) -> bool {
        self.collapsed.is_some()
    }

    /// Get the age of this faction (years since founding)
    pub fn age(&self) -> i32 {
        if let Some(collapse) = self.collapsed {
            collapse.0 - self.founded.0
        } else {
            -self.founded.0 // If still active, age is years since founding
        }
    }

    /// Get years since collapse (0 if still active)
    pub fn years_collapsed(&self) -> i32 {
        self.collapsed.map(|y| -y.0).unwrap_or(0)
    }
}

/// Collection of all factions and their relationships
#[derive(Clone, Debug)]
pub struct FactionRegistry {
    /// All factions by ID
    pub factions: HashMap<FactionId, Faction>,
    /// Relationships between factions (faction pair -> relation value -1.0 to 1.0)
    pub relationships: HashMap<(FactionId, FactionId), f32>,
    /// Next available faction ID
    next_id: u32,
}

impl Default for FactionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FactionRegistry {
    pub fn new() -> Self {
        Self {
            factions: HashMap::new(),
            relationships: HashMap::new(),
            next_id: 0,
        }
    }

    /// Add a faction to the registry
    pub fn add(&mut self, faction: Faction) {
        self.factions.insert(faction.id, faction);
    }

    /// Get a faction by ID
    pub fn get(&self, id: FactionId) -> Option<&Faction> {
        self.factions.get(&id)
    }

    /// Get all factions
    pub fn all(&self) -> impl Iterator<Item = &Faction> {
        self.factions.values()
    }

    /// Get all active (non-collapsed) factions
    pub fn active(&self) -> impl Iterator<Item = &Faction> {
        self.factions.values().filter(|f| !f.is_collapsed())
    }

    /// Get all collapsed factions
    pub fn collapsed(&self) -> impl Iterator<Item = &Faction> {
        self.factions.values().filter(|f| f.is_collapsed())
    }

    /// Get the relationship between two factions
    pub fn relationship(&self, a: FactionId, b: FactionId) -> FactionRelation {
        let key = if a.0 < b.0 { (a, b) } else { (b, a) };
        let value = self.relationships.get(&key).copied().unwrap_or(0.0);
        FactionRelation::from_value(value)
    }

    /// Set the relationship between two factions
    pub fn set_relationship(&mut self, a: FactionId, b: FactionId, value: f32) {
        let key = if a.0 < b.0 { (a, b) } else { (b, a) };
        self.relationships.insert(key, value.clamp(-1.0, 1.0));
    }

    /// Generate a new unique faction ID
    pub fn new_id(&mut self) -> FactionId {
        let id = FactionId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Get factions that were at war
    pub fn wars(&self) -> Vec<(FactionId, FactionId)> {
        self.relationships
            .iter()
            .filter(|(_, &v)| v < -0.7)
            .map(|(&k, _)| k)
            .collect()
    }
}

/// Generate factions for a world
pub fn generate_factions(
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    seed: u64,
) -> FactionRegistry {
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0xFAC710A5));
    let name_gen = NameGenerator::new(seed);
    let mut registry = FactionRegistry::new();

    // Determine number of factions based on map size
    let map_area = heightmap.width * heightmap.height;
    let base_factions = 8;
    let scale = (map_area as f32 / (512.0 * 256.0)).sqrt();
    let num_factions = ((base_factions as f32 * scale) as usize).clamp(6, 20);

    println!("  Generating {} factions...", num_factions);

    // Track which species are already used (limit duplicates)
    let mut species_counts: HashMap<Species, u32> = HashMap::new();

    for i in 0..num_factions {
        // Pick a species (weighted by remaining capacity)
        let species = pick_species(&mut rng, &species_counts);
        *species_counts.entry(species).or_insert(0) += 1;

        // Pick culture (weighted by species tendencies)
        let culture = pick_culture(species, &mut rng);

        // Architecture defaults to species-appropriate but can vary
        let architecture = if rng.gen_bool(0.7) {
            ArchitectureStyle::default_for_species(species)
        } else {
            *pick_random(&mut rng, ArchitectureStyle::all())
        };

        // Generate faction name
        let name = name_gen.faction_name(species, culture, &mut rng);

        // Generate founding year (older civilizations = more ruins)
        let age = rng.gen_range(200..3000);
        let founded = Year::years_ago(age);

        // Determine if faction collapsed
        let (collapsed, collapse_reason) = if rng.gen_bool(0.7) {
            // 70% of factions have collapsed
            let collapse_age = rng.gen_range(50..age - 50).max(50);
            let reason = pick_collapse_reason(&mut rng);
            (Some(Year::years_ago(collapse_age)), Some(reason))
        } else {
            (None, None)
        };

        // Generate faction color
        let hue = (i as f32 / num_factions as f32) * 360.0 + rng.gen_range(-20.0..20.0);
        let color = hsv_to_rgb(hue % 360.0, 0.7, 0.8);

        // Estimate peak size
        let base_settlements = match culture {
            CultureType::Expansionist => rng.gen_range(8..20),
            CultureType::Mercantile => rng.gen_range(5..12),
            CultureType::Isolationist => rng.gen_range(2..6),
            CultureType::Nomadic => rng.gen_range(1..4),
            _ => rng.gen_range(3..10),
        };

        let peak_population = base_settlements * rng.gen_range(1000..10000);

        let id = registry.new_id();
        let faction = Faction {
            id,
            name,
            species,
            culture,
            architecture,
            founded,
            collapsed,
            collapse_reason,
            color,
            capital: None, // Set later during settlement generation
            peak_settlements: base_settlements,
            peak_population,
        };

        registry.add(faction);
    }

    // Generate relationships between factions
    generate_relationships(&mut registry, &mut rng);

    registry
}

/// Pick a species for a new faction
fn pick_species(rng: &mut ChaCha8Rng, counts: &HashMap<Species, u32>) -> Species {
    let species = Species::all();
    let weights: Vec<f32> = species.iter().map(|s| {
        let count = counts.get(s).copied().unwrap_or(0);
        // Base weight (humans more common, rare species less common)
        let base = match s {
            Species::Human => 3.0,
            Species::Dwarf => 2.0,
            Species::Elf => 2.0,
            Species::Orc => 1.5,
            Species::Goblin => 1.0,
            Species::Giant => 0.5,
            Species::DragonKin => 0.3,
            Species::Undead => 0.5,
            Species::Elemental => 0.3,
        };
        // Reduce weight if already many of this species
        base / (1.0 + count as f32)
    }).collect();

    let total: f32 = weights.iter().sum();
    let mut r = rng.gen::<f32>() * total;

    for (i, weight) in weights.iter().enumerate() {
        r -= weight;
        if r <= 0.0 {
            return species[i];
        }
    }

    Species::Human // Fallback
}

/// Pick a culture type based on species tendencies
fn pick_culture(species: Species, rng: &mut ChaCha8Rng) -> CultureType {
    pick_culture_biome(species, None, rng)
}

/// Pick a culture type based on species tendencies and biome influence
/// Biome can push cultures in certain directions (desert -> nomadic, coastal -> mercantile, etc.)
pub fn pick_culture_biome(species: Species, biome_category: Option<BiomeCategory>, rng: &mut ChaCha8Rng) -> CultureType {
    let mut weights: Vec<(CultureType, f32)> = match species {
        Species::Human => vec![
            (CultureType::Expansionist, 2.0),
            (CultureType::Mercantile, 2.0),
            (CultureType::Militaristic, 1.5),
            (CultureType::Religious, 1.0),
            (CultureType::Scholarly, 1.0),
            (CultureType::Nomadic, 0.5),
            (CultureType::Industrial, 0.5),
            (CultureType::Isolationist, 0.5),
        ],
        Species::Dwarf => vec![
            (CultureType::Industrial, 3.0),
            (CultureType::Isolationist, 2.0),
            (CultureType::Mercantile, 1.0),
            (CultureType::Militaristic, 0.5),
        ],
        Species::Elf => vec![
            (CultureType::Isolationist, 2.5),
            (CultureType::Scholarly, 2.0),
            (CultureType::Religious, 1.0),
        ],
        Species::Orc => vec![
            (CultureType::Militaristic, 3.0),
            (CultureType::Nomadic, 2.0),
            (CultureType::Expansionist, 1.0),
        ],
        Species::Goblin => vec![
            (CultureType::Nomadic, 2.0),
            (CultureType::Industrial, 1.5),
            (CultureType::Mercantile, 1.0),
        ],
        Species::Giant => vec![
            (CultureType::Isolationist, 2.0),
            (CultureType::Nomadic, 2.0),
            (CultureType::Militaristic, 1.0),
        ],
        Species::DragonKin => vec![
            (CultureType::Religious, 2.0),
            (CultureType::Militaristic, 2.0),
            (CultureType::Isolationist, 1.0),
        ],
        Species::Undead => vec![
            (CultureType::Religious, 2.0),
            (CultureType::Expansionist, 2.0),
            (CultureType::Scholarly, 1.0),
        ],
        Species::Elemental => vec![
            (CultureType::Isolationist, 2.0),
            (CultureType::Religious, 1.5),
            (CultureType::Nomadic, 1.0),
        ],
    };

    // Apply biome modifiers
    if let Some(biome) = biome_category {
        let modifiers = biome_culture_modifiers(biome);
        for (culture, modifier) in modifiers {
            if let Some(entry) = weights.iter_mut().find(|(c, _)| *c == culture) {
                entry.1 += modifier;
            } else {
                weights.push((culture, modifier));
            }
        }
        // Ensure no negative weights
        for entry in weights.iter_mut() {
            entry.1 = entry.1.max(0.0);
        }
    }

    let total: f32 = weights.iter().map(|(_, w)| w).sum();
    if total <= 0.0 {
        return CultureType::Militaristic;
    }
    let mut r = rng.gen::<f32>() * total;

    for (culture, weight) in weights {
        r -= weight;
        if r <= 0.0 {
            return culture;
        }
    }

    CultureType::Militaristic // Fallback
}

/// Get culture weight modifiers based on biome
fn biome_culture_modifiers(category: BiomeCategory) -> Vec<(CultureType, f32)> {
    match category {
        BiomeCategory::Desert => vec![
            (CultureType::Nomadic, 2.0),
            (CultureType::Mercantile, 1.0), // Trade routes through desert
            (CultureType::Isolationist, -0.5),
        ],
        BiomeCategory::Mountain | BiomeCategory::Volcanic => vec![
            (CultureType::Industrial, 2.0), // Mining, forging
            (CultureType::Isolationist, 1.5),
            (CultureType::Nomadic, -1.0),
        ],
        BiomeCategory::Coastal => vec![
            (CultureType::Mercantile, 2.0), // Sea trade
            (CultureType::Expansionist, 1.0), // Naval expansion
            (CultureType::Isolationist, -0.5),
        ],
        BiomeCategory::Forest => vec![
            (CultureType::Isolationist, 1.5),
            (CultureType::Scholarly, 1.0), // Druids, nature wisdom
            (CultureType::Industrial, -1.0),
        ],
        BiomeCategory::Grassland => vec![
            (CultureType::Expansionist, 1.5),
            (CultureType::Nomadic, 1.0),
            (CultureType::Militaristic, 0.5),
        ],
        BiomeCategory::Swamp => vec![
            (CultureType::Isolationist, 2.0),
            (CultureType::Religious, 1.0), // Swamp cults
            (CultureType::Mercantile, -1.0),
        ],
        BiomeCategory::Tundra => vec![
            (CultureType::Nomadic, 2.0),
            (CultureType::Isolationist, 1.0),
            (CultureType::Expansionist, -1.0),
        ],
        BiomeCategory::Hills => vec![
            (CultureType::Nomadic, 1.0),
            (CultureType::Industrial, 0.5), // Quarries
        ],
        BiomeCategory::Cave => vec![
            (CultureType::Industrial, 2.0), // Underground mining
            (CultureType::Isolationist, 1.5),
            (CultureType::Nomadic, -1.0),
        ],
        BiomeCategory::Ruins => vec![
            (CultureType::Scholarly, 2.0), // Studying the past
            (CultureType::Religious, 1.0), // Ancestor worship
        ],
        BiomeCategory::Mystical => vec![
            (CultureType::Scholarly, 2.0), // Arcane studies
            (CultureType::Religious, 1.5), // Mystical worship
            (CultureType::Industrial, -1.0),
        ],
        BiomeCategory::Ocean => vec![
            (CultureType::Mercantile, 1.5),
            (CultureType::Isolationist, 1.0),
        ],
    }
}

/// Pick a reason for faction collapse
fn pick_collapse_reason(rng: &mut ChaCha8Rng) -> AbandonmentReason {
    let reasons = [
        (AbandonmentReason::Conquest, 25),
        (AbandonmentReason::War, 20),
        (AbandonmentReason::Plague, 15),
        (AbandonmentReason::MonsterAttack, 10),
        (AbandonmentReason::NaturalDisaster, 10),
        (AbandonmentReason::FactionCollapse, 10),
        (AbandonmentReason::ResourceDepletion, 5),
        (AbandonmentReason::Unknown, 5),
    ];

    let total: u32 = reasons.iter().map(|(_, w)| w).sum();
    let mut r = rng.gen_range(0..total);

    for (reason, weight) in reasons {
        if r < weight {
            return reason;
        }
        r -= weight;
    }

    AbandonmentReason::Unknown
}

/// Generate relationships between all factions
fn generate_relationships(registry: &mut FactionRegistry, rng: &mut ChaCha8Rng) {
    let faction_ids: Vec<FactionId> = registry.factions.keys().copied().collect();

    for i in 0..faction_ids.len() {
        for j in (i + 1)..faction_ids.len() {
            let a = faction_ids[i];
            let b = faction_ids[j];

            let faction_a = registry.factions.get(&a).unwrap();
            let faction_b = registry.factions.get(&b).unwrap();

            // Base relationship based on species
            let species_factor = species_relationship(faction_a.species, faction_b.species);

            // Culture compatibility
            let culture_factor = culture_relationship(faction_a.culture, faction_b.culture);

            // Add some randomness
            let random_factor = rng.gen_range(-0.3..0.3);

            // Combine factors
            let relationship = (species_factor * 0.5 + culture_factor * 0.3 + random_factor).clamp(-1.0, 1.0);

            registry.set_relationship(a, b, relationship);
        }
    }
}

/// Get base relationship between two species
fn species_relationship(a: Species, b: Species) -> f32 {
    if a == b {
        return 0.5; // Same species = friendly
    }

    match (a, b) {
        // Natural allies
        (Species::Human, Species::Elf) | (Species::Elf, Species::Human) => 0.3,
        (Species::Human, Species::Dwarf) | (Species::Dwarf, Species::Human) => 0.4,
        (Species::Elf, Species::Dwarf) | (Species::Dwarf, Species::Elf) => 0.1,

        // Natural enemies
        (Species::Human, Species::Orc) | (Species::Orc, Species::Human) => -0.6,
        (Species::Elf, Species::Orc) | (Species::Orc, Species::Elf) => -0.7,
        (Species::Dwarf, Species::Goblin) | (Species::Goblin, Species::Dwarf) => -0.6,
        (Species::Dwarf, Species::Orc) | (Species::Orc, Species::Dwarf) => -0.5,

        // Undead are generally hostile to all
        (Species::Undead, _) | (_, Species::Undead) => -0.5,

        // Orcs and Goblins sometimes work together
        (Species::Orc, Species::Goblin) | (Species::Goblin, Species::Orc) => 0.2,

        // Elementals are neutral to most
        (Species::Elemental, _) | (_, Species::Elemental) => 0.0,

        // Giants don't care much about smaller races
        (Species::Giant, _) | (_, Species::Giant) => -0.1,

        // DragonKin are arrogant
        (Species::DragonKin, _) | (_, Species::DragonKin) => -0.2,

        // Default neutral
        _ => 0.0,
    }
}

/// Get relationship modifier based on culture compatibility
fn culture_relationship(a: CultureType, b: CultureType) -> f32 {
    if a == b {
        return 0.3; // Same culture = friendly
    }

    match (a, b) {
        // Mercantile likes other trading cultures
        (CultureType::Mercantile, CultureType::Industrial) |
        (CultureType::Industrial, CultureType::Mercantile) => 0.3,

        // Scholarly respects religious
        (CultureType::Scholarly, CultureType::Religious) |
        (CultureType::Religious, CultureType::Scholarly) => 0.2,

        // Militaristic clashes with others
        (CultureType::Militaristic, CultureType::Mercantile) |
        (CultureType::Mercantile, CultureType::Militaristic) => -0.2,

        // Expansionist clashes with Isolationist
        (CultureType::Expansionist, CultureType::Isolationist) |
        (CultureType::Isolationist, CultureType::Expansionist) => -0.4,

        // Nomadic is neutral with most
        (CultureType::Nomadic, _) | (_, CultureType::Nomadic) => 0.0,

        _ => 0.0,
    }
}

/// Helper to pick a random element from a slice
fn pick_random<'a, T>(rng: &mut ChaCha8Rng, items: &'a [T]) -> &'a T {
    &items[rng.gen_range(0..items.len())]
}

/// Convert HSV to RGB
fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_faction_generation() {
        let heightmap = Tilemap::new_with(64, 32, 100.0f32);
        let biomes = Tilemap::new_with(64, 32, ExtendedBiome::TemperateGrassland);

        let registry = generate_factions(&heightmap, &biomes, 42);

        assert!(registry.factions.len() >= 6, "Should generate at least 6 factions");

        for faction in registry.all() {
            assert!(!faction.name.is_empty(), "Faction should have a name");
            println!("{}: {} {} ({})",
                faction.name,
                faction.species.name(),
                faction.culture.name(),
                if faction.is_collapsed() { "collapsed" } else { "active" }
            );
        }
    }
}
