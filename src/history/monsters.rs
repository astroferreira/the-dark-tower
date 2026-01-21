//! Monster ecology and lair generation
//!
//! Places monster lairs based on terrain preferences and creates territory markers.

use std::collections::HashMap;

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use crate::biomes::ExtendedBiome;
use crate::tilemap::Tilemap;

use super::naming::NameGenerator;
use super::types::*;

/// Species of monsters that create lairs
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MonsterSpecies {
    // Surface monsters
    GiantSpider,
    Troll,
    Ogre,
    Wyvern,
    Harpy,
    Werewolf,

    // Underground monsters
    CaveCrawler,
    DarkElf,
    DeepWorm,

    // Magical monsters
    Dragon,
    Elemental,
    Wraith,
    Lich,

    // Swarm monsters
    GiantAnt,
    GiantBee,
    GoblinBand,
}

impl MonsterSpecies {
    pub fn all() -> &'static [MonsterSpecies] {
        &[
            MonsterSpecies::GiantSpider,
            MonsterSpecies::Troll,
            MonsterSpecies::Ogre,
            MonsterSpecies::Wyvern,
            MonsterSpecies::Harpy,
            MonsterSpecies::Werewolf,
            MonsterSpecies::CaveCrawler,
            MonsterSpecies::DarkElf,
            MonsterSpecies::DeepWorm,
            MonsterSpecies::Dragon,
            MonsterSpecies::Elemental,
            MonsterSpecies::Wraith,
            MonsterSpecies::Lich,
            MonsterSpecies::GiantAnt,
            MonsterSpecies::GiantBee,
            MonsterSpecies::GoblinBand,
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            MonsterSpecies::GiantSpider => "Giant Spider",
            MonsterSpecies::Troll => "Troll",
            MonsterSpecies::Ogre => "Ogre",
            MonsterSpecies::Wyvern => "Wyvern",
            MonsterSpecies::Harpy => "Harpy",
            MonsterSpecies::Werewolf => "Werewolf",
            MonsterSpecies::CaveCrawler => "Cave Crawler",
            MonsterSpecies::DarkElf => "Dark Elf",
            MonsterSpecies::DeepWorm => "Deep Worm",
            MonsterSpecies::Dragon => "Dragon",
            MonsterSpecies::Elemental => "Elemental",
            MonsterSpecies::Wraith => "Wraith",
            MonsterSpecies::Lich => "Lich",
            MonsterSpecies::GiantAnt => "Giant Ant Colony",
            MonsterSpecies::GiantBee => "Giant Bee Hive",
            MonsterSpecies::GoblinBand => "Goblin Band",
        }
    }

    /// Whether this monster lives underground
    pub fn is_underground(&self) -> bool {
        matches!(self,
            MonsterSpecies::CaveCrawler | MonsterSpecies::DarkElf |
            MonsterSpecies::DeepWorm | MonsterSpecies::GiantAnt
        )
    }

    /// Preferred biomes for this monster
    pub fn preferred_biomes(&self) -> &'static [BiomeCategory] {
        match self {
            MonsterSpecies::GiantSpider => &[BiomeCategory::Forest, BiomeCategory::Swamp, BiomeCategory::Cave],
            MonsterSpecies::Troll => &[BiomeCategory::Swamp, BiomeCategory::Mountain, BiomeCategory::Forest],
            MonsterSpecies::Ogre => &[BiomeCategory::Hills, BiomeCategory::Forest, BiomeCategory::Mountain],
            MonsterSpecies::Wyvern => &[BiomeCategory::Mountain, BiomeCategory::Hills],
            MonsterSpecies::Harpy => &[BiomeCategory::Mountain, BiomeCategory::Coastal],
            MonsterSpecies::Werewolf => &[BiomeCategory::Forest, BiomeCategory::Hills],
            MonsterSpecies::CaveCrawler => &[BiomeCategory::Cave],
            MonsterSpecies::DarkElf => &[BiomeCategory::Cave],
            MonsterSpecies::DeepWorm => &[BiomeCategory::Cave],
            MonsterSpecies::Dragon => &[BiomeCategory::Mountain, BiomeCategory::Volcanic],
            MonsterSpecies::Elemental => &[BiomeCategory::Volcanic, BiomeCategory::Desert, BiomeCategory::Tundra],
            MonsterSpecies::Wraith => &[BiomeCategory::Swamp, BiomeCategory::Ruins],
            MonsterSpecies::Lich => &[BiomeCategory::Ruins, BiomeCategory::Cave],
            MonsterSpecies::GiantAnt => &[BiomeCategory::Desert, BiomeCategory::Grassland, BiomeCategory::Cave],
            MonsterSpecies::GiantBee => &[BiomeCategory::Forest, BiomeCategory::Grassland],
            MonsterSpecies::GoblinBand => &[BiomeCategory::Hills, BiomeCategory::Cave, BiomeCategory::Forest],
        }
    }

    /// Danger level (1-10)
    pub fn danger_level(&self) -> u8 {
        match self {
            MonsterSpecies::GiantSpider => 3,
            MonsterSpecies::Troll => 5,
            MonsterSpecies::Ogre => 4,
            MonsterSpecies::Wyvern => 6,
            MonsterSpecies::Harpy => 3,
            MonsterSpecies::Werewolf => 5,
            MonsterSpecies::CaveCrawler => 4,
            MonsterSpecies::DarkElf => 6,
            MonsterSpecies::DeepWorm => 7,
            MonsterSpecies::Dragon => 10,
            MonsterSpecies::Elemental => 7,
            MonsterSpecies::Wraith => 6,
            MonsterSpecies::Lich => 9,
            MonsterSpecies::GiantAnt => 4,
            MonsterSpecies::GiantBee => 3,
            MonsterSpecies::GoblinBand => 4,
        }
    }

    /// Territory radius (in tiles)
    pub fn territory_radius(&self) -> usize {
        match self {
            MonsterSpecies::Dragon => 30,
            MonsterSpecies::DeepWorm => 25,
            MonsterSpecies::Lich => 20,
            MonsterSpecies::GiantAnt => 20,
            MonsterSpecies::Wyvern => 15,
            MonsterSpecies::Troll | MonsterSpecies::Ogre => 10,
            _ => 8,
        }
    }

    /// Type of evidence this monster leaves
    pub fn territory_evidence(&self) -> &'static str {
        match self {
            MonsterSpecies::GiantSpider => "WebCluster",
            MonsterSpecies::Troll => "BoneNest",
            MonsterSpecies::Ogre => "BoneNest",
            MonsterSpecies::Dragon => "CharredGround",
            MonsterSpecies::GiantAnt => "AntMound",
            MonsterSpecies::GiantBee => "BeeHive",
            MonsterSpecies::Werewolf => "ClawMarks",
            MonsterSpecies::Wraith | MonsterSpecies::Lich => "CursedGround",
            _ => "TerritoryMarking",
        }
    }
}

/// Biome category for monster placement and naming
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BiomeCategory {
    Forest,
    Mountain,
    Swamp,
    Desert,
    Tundra,
    Grassland,
    Hills,
    Coastal,
    Volcanic,
    Cave,
    Ruins,
    Ocean,
    Mystical,
}

/// A monster lair
#[derive(Clone, Debug)]
pub struct MonsterLair {
    /// Unique identifier
    pub id: LairId,
    /// Monster species
    pub species: MonsterSpecies,
    /// Location
    pub x: usize,
    pub y: usize,
    /// Z-level (for underground lairs)
    pub z: i32,
    /// Name of the lair (e.g., "Shadowfang's Den")
    pub name: String,
    /// Whether this lair is active
    pub active: bool,
    /// Danger level (modified by age, size, etc.)
    pub danger: u8,
    /// Territory tiles
    pub territory: Vec<(usize, usize)>,
    /// Historical attacks on nearby settlements
    pub attacks: Vec<(Year, String)>,
    /// Artifacts hoarded by this monster
    pub hoard: Vec<ArtifactId>,
    /// How the monster acquired artifacts (id, description)
    pub hoard_sources: Vec<(ArtifactId, String)>,
}

/// Registry of monster lairs
#[derive(Clone, Debug)]
pub struct MonsterRegistry {
    /// All lairs by ID
    pub lairs: HashMap<LairId, MonsterLair>,
    /// Lairs by location
    pub lairs_by_location: HashMap<(usize, usize), LairId>,
    /// Next lair ID
    next_id: u32,
}

impl Default for MonsterRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl MonsterRegistry {
    pub fn new() -> Self {
        Self {
            lairs: HashMap::new(),
            lairs_by_location: HashMap::new(),
            next_id: 0,
        }
    }

    /// Add a lair
    pub fn add(&mut self, lair: MonsterLair) {
        let id = lair.id;
        let loc = (lair.x, lair.y);
        self.lairs_by_location.insert(loc, id);
        self.lairs.insert(id, lair);
    }

    /// Get lair at location
    pub fn lair_at(&self, x: usize, y: usize) -> Option<&MonsterLair> {
        self.lairs_by_location.get(&(x, y))
            .and_then(|id| self.lairs.get(id))
    }

    /// Generate new lair ID
    pub fn new_id(&mut self) -> LairId {
        let id = LairId(self.next_id);
        self.next_id += 1;
        id
    }

    /// Get all active lairs
    pub fn active_lairs(&self) -> impl Iterator<Item = &MonsterLair> {
        self.lairs.values().filter(|l| l.active)
    }
}

/// Generate monster lairs for the world
pub fn generate_monster_lairs(
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    stress_map: &Tilemap<f32>,
    seed: u64,
) -> MonsterRegistry {
    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0xB0A5BE25));
    let name_gen = NameGenerator::new(seed);
    let mut registry = MonsterRegistry::new();

    let width = heightmap.width;
    let height = heightmap.height;

    // Scale number of lairs with map size
    let map_area = width * height;
    let base_lairs = 10;
    let scale = (map_area as f32 / (512.0 * 256.0)).sqrt();
    let num_lairs = ((base_lairs as f32 * scale) as usize).clamp(5, 30);

    println!("  Generating {} monster lairs...", num_lairs);

    // Track used locations to avoid overlap
    let mut used: Vec<(usize, usize)> = Vec::new();

    for _ in 0..num_lairs {
        // Pick a monster species
        let species = pick_monster_species(&mut rng);

        // Find suitable location
        let location = find_lair_location(
            species,
            heightmap,
            biomes,
            stress_map,
            &used,
            width,
            height,
            &mut rng,
        );

        if let Some((x, y)) = location {
            // Mark area as used
            let radius = species.territory_radius();
            for dy in 0..radius {
                for dx in 0..radius {
                    for (sx, sy) in [
                        (x.wrapping_add(dx), y.wrapping_add(dy)),
                        (x.wrapping_sub(dx), y.wrapping_add(dy)),
                        (x.wrapping_add(dx), y.wrapping_sub(dy)),
                        (x.wrapping_sub(dx), y.wrapping_sub(dy)),
                    ] {
                        if sx < width && sy < height {
                            used.push((sx, sy));
                        }
                    }
                }
            }

            // Generate biome-aware lair name
            let biome = *biomes.get(x, y);
            let biome_category = categorize_biome(biome);
            let name = generate_lair_name(species, biome_category, &name_gen, &mut rng);

            // Determine if lair is active
            let active = rng.gen_bool(0.7);

            // Calculate territory
            let territory = generate_territory(x, y, radius, width, height);

            // Generate attack history
            let attacks = if rng.gen_bool(0.5) {
                let num_attacks = rng.gen_range(1..=3);
                (0..num_attacks).map(|_| {
                    let years_ago = rng.gen_range(10..500);
                    let target = name_gen.settlement_name(Species::Human, &mut rng);
                    (Year::years_ago(years_ago), target)
                }).collect()
            } else {
                Vec::new()
            };

            let id = registry.new_id();
            let lair = MonsterLair {
                id,
                species,
                x,
                y,
                z: if species.is_underground() { rng.gen_range(-10..-3) } else { 0 },
                name,
                active,
                danger: species.danger_level() + rng.gen_range(0..=2),
                territory,
                attacks,
                hoard: Vec::new(),
                hoard_sources: Vec::new(),
            };

            registry.add(lair);
        }
    }

    registry
}

/// Pick a monster species
fn pick_monster_species(rng: &mut ChaCha8Rng) -> MonsterSpecies {
    let species = MonsterSpecies::all();
    let weights: Vec<u32> = species.iter().map(|s| {
        // Rarer monsters = lower weight
        match s {
            MonsterSpecies::Dragon => 5,
            MonsterSpecies::Lich => 8,
            MonsterSpecies::DeepWorm => 10,
            MonsterSpecies::Elemental => 12,
            MonsterSpecies::GiantSpider | MonsterSpecies::Troll | MonsterSpecies::Ogre => 25,
            _ => 20,
        }
    }).collect();

    let total: u32 = weights.iter().sum();
    let mut r = rng.gen_range(0..total);

    for (i, &weight) in weights.iter().enumerate() {
        if r < weight {
            return species[i];
        }
        r -= weight;
    }

    MonsterSpecies::GiantSpider
}

/// Find a suitable location for a monster lair
fn find_lair_location(
    species: MonsterSpecies,
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    stress_map: &Tilemap<f32>,
    used: &[(usize, usize)],
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) -> Option<(usize, usize)> {
    let preferred = species.preferred_biomes();
    let mut candidates: Vec<(usize, usize, f32)> = Vec::new();

    for y in 5..(height - 5) {
        for x in 5..(width - 5) {
            // Skip used locations
            if used.contains(&(x, y)) {
                continue;
            }

            let elev = *heightmap.get(x, y);

            // Skip water
            if elev < 0.0 {
                continue;
            }

            let biome = *biomes.get(x, y);
            let category = categorize_biome(biome);

            // Check if this biome is preferred
            if !preferred.contains(&category) && category != BiomeCategory::Ruins {
                continue;
            }

            // Score based on remoteness (higher elevation = more remote)
            let mut score = 0.5;

            // Mountains and hills are good for most monsters
            if elev > 1000.0 {
                score += 0.3;
            }

            // Volcanic areas for dragons/elementals
            if matches!(species, MonsterSpecies::Dragon | MonsterSpecies::Elemental) {
                let stress = *stress_map.get(x, y);
                if stress > 0.5 {
                    score += 0.4;
                }
            }

            candidates.push((x, y, score));
        }
    }

    if candidates.is_empty() {
        return None;
    }

    candidates.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
    let top_n = candidates.len().min(20);
    let idx = rng.gen_range(0..top_n);

    Some((candidates[idx].0, candidates[idx].1))
}

/// Categorize a biome for monster placement, naming, and culture selection.
/// This function is public so other modules can use it for biome-aware generation.
pub fn categorize_biome(biome: ExtendedBiome) -> BiomeCategory {
    match biome {
        // === FORESTS ===
        ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest |
        ExtendedBiome::TropicalForest | ExtendedBiome::TropicalRainforest |
        ExtendedBiome::TemperateRainforest |
        // Fantasy forests
        ExtendedBiome::DeadForest | ExtendedBiome::CrystalForest |
        ExtendedBiome::BioluminescentForest | ExtendedBiome::MushroomForest |
        ExtendedBiome::PetrifiedForest |
        // Ancient/Primeval forests
        ExtendedBiome::AncientGrove |
        // Biological
        ExtendedBiome::FungalBloom |
        // Alien
        ExtendedBiome::SiliconGrove => BiomeCategory::Forest,

        // === MOUNTAINS ===
        ExtendedBiome::AlpineTundra | ExtendedBiome::SnowyPeaks |
        ExtendedBiome::RazorPeaks |
        // Extreme geological
        ExtendedBiome::BasaltColumns => BiomeCategory::Mountain,

        // === SWAMPS/WETLANDS ===
        ExtendedBiome::Swamp | ExtendedBiome::MangroveSaltmarsh | ExtendedBiome::Marsh |
        ExtendedBiome::Bog |
        // Mystical wetlands
        ExtendedBiome::Shadowfen | ExtendedBiome::SpiritMarsh |
        // Biological
        ExtendedBiome::CarnivorousBog |
        // Fantasy waters
        ExtendedBiome::AcidLake => BiomeCategory::Swamp,

        // === DESERT ===
        ExtendedBiome::Desert | ExtendedBiome::SingingDunes | ExtendedBiome::SaltFlats |
        ExtendedBiome::Oasis | ExtendedBiome::GlassDesert |
        // Wastelands (hot/dry)
        ExtendedBiome::CrystalWasteland |
        // Alien/Corrupted (dry)
        ExtendedBiome::SporeWastes => BiomeCategory::Desert,

        // === TUNDRA/ICE ===
        ExtendedBiome::Tundra | ExtendedBiome::Ice | ExtendedBiome::AuroraWastes |
        // Fantasy waters (frozen)
        ExtendedBiome::FrozenLake |
        // Ocean (frozen)
        ExtendedBiome::FrozenAbyss => BiomeCategory::Tundra,

        // === GRASSLAND ===
        ExtendedBiome::TemperateGrassland | ExtendedBiome::Foothills | ExtendedBiome::Savanna => BiomeCategory::Grassland,

        // === HILLS ===
        ExtendedBiome::PaintedHills |
        // Karst
        ExtendedBiome::KarstPlains | ExtendedBiome::TowerKarst | ExtendedBiome::CockpitKarst => BiomeCategory::Hills,

        // === COASTAL ===
        ExtendedBiome::Lagoon | ExtendedBiome::CoastalWater | ExtendedBiome::CoralReef |
        // Realistic shallow
        ExtendedBiome::KelpForest | ExtendedBiome::SeagrassMeadow |
        // Fantasy shallow
        ExtendedBiome::SirenShallows | ExtendedBiome::PearlGardens |
        // Volcanic coastal
        ExtendedBiome::VolcanicBeach |
        // Aquatic features
        ExtendedBiome::Sargasso |
        // Exotic waters
        ExtendedBiome::PhosphorShallows => BiomeCategory::Coastal,

        // === VOLCANIC ===
        ExtendedBiome::VolcanicWasteland | ExtendedBiome::LavaLake |
        // Geothermal
        ExtendedBiome::ObsidianFields | ExtendedBiome::Geysers | ExtendedBiome::TarPits |
        ExtendedBiome::SulfurVents |
        // Wastelands (volcanic)
        ExtendedBiome::Ashlands |
        // Volcanic biomes
        ExtendedBiome::Caldera | ExtendedBiome::ShieldVolcano | ExtendedBiome::VolcanicCone |
        ExtendedBiome::LavaField | ExtendedBiome::FumaroleField | ExtendedBiome::HotSpot |
        // Deep ocean thermal
        ExtendedBiome::AbyssalVents | ExtendedBiome::ThermalVents |
        // Exotic waters (hot)
        ExtendedBiome::HotSprings => BiomeCategory::Volcanic,

        // === CAVE ===
        ExtendedBiome::CaveEntrance | ExtendedBiome::Sinkhole | ExtendedBiome::Cenote |
        ExtendedBiome::SinkholeLakes |
        // Biological (underground)
        ExtendedBiome::ColossalHive |
        // Alien (underground)
        ExtendedBiome::HollowEarth |
        // Deep ocean cave-like
        ExtendedBiome::ColdSeep | ExtendedBiome::BrinePool => BiomeCategory::Cave,

        // === RUINS ===
        ExtendedBiome::SunkenCity | ExtendedBiome::CyclopeanRuins |
        ExtendedBiome::BuriedTemple | ExtendedBiome::OvergrownCitadel |
        ExtendedBiome::DarkTower |
        // Bone/Death
        ExtendedBiome::TitanBones | ExtendedBiome::BoneFields |
        // Deep ocean ruins
        ExtendedBiome::DrownedCitadel | ExtendedBiome::LeviathanGraveyard => BiomeCategory::Ruins,

        // === OCEAN ===
        ExtendedBiome::DeepOcean | ExtendedBiome::Ocean |
        // Realistic mid-depth
        ExtendedBiome::ContinentalShelf | ExtendedBiome::Seamount |
        // Realistic deep
        ExtendedBiome::OceanicTrench | ExtendedBiome::AbyssalPlain | ExtendedBiome::MidOceanRidge |
        // Fantasy deep
        ExtendedBiome::CrystalDepths | ExtendedBiome::VoidMaw |
        // Exotic waters (ocean)
        ExtendedBiome::BrinePools | ExtendedBiome::InkSea => BiomeCategory::Ocean,

        // === MYSTICAL ===
        ExtendedBiome::EtherealMist | ExtendedBiome::StarfallCrater |
        ExtendedBiome::LeyNexus | ExtendedBiome::WhisperingStones |
        // Magical/Anomalous
        ExtendedBiome::FloatingStones | ExtendedBiome::PrismaticPools |
        // Fantasy waters (magical)
        ExtendedBiome::BioluminescentWater | ExtendedBiome::MirrorLake |
        // Alien/Corrupted (magical)
        ExtendedBiome::VoidScar | ExtendedBiome::BleedingStone |
        // Ancient (magical)
        ExtendedBiome::CoralPlateau |
        // Kelp towers
        ExtendedBiome::KelpTowers => BiomeCategory::Mystical,

        // === DEFAULT (remaining biomes -> Grassland) ===
        _ => BiomeCategory::Grassland,
    }
}

/// Generate a name for a monster lair (biome-aware)
fn generate_lair_name(species: MonsterSpecies, biome_category: BiomeCategory, name_gen: &NameGenerator, rng: &mut ChaCha8Rng) -> String {
    // 60% chance to use biome-aware naming
    if rng.gen_bool(0.6) {
        return name_gen.lair_name_biome(biome_category, rng);
    }

    let location_type = match species {
        MonsterSpecies::GiantSpider => pick_random(rng, &["Web", "Nest", "Lair", "Den"]),
        MonsterSpecies::Troll | MonsterSpecies::Ogre => pick_random(rng, &["Cave", "Den", "Lair", "Pit"]),
        MonsterSpecies::Wyvern | MonsterSpecies::Harpy => pick_random(rng, &["Aerie", "Roost", "Peak", "Nest"]),
        MonsterSpecies::Dragon => pick_random(rng, &["Lair", "Domain", "Peak", "Sanctum"]),
        MonsterSpecies::Wraith | MonsterSpecies::Lich => pick_random(rng, &["Tomb", "Crypt", "Barrow", "Sanctum"]),
        MonsterSpecies::GiantAnt => pick_random(rng, &["Colony", "Mound", "Nest", "Warren"]),
        MonsterSpecies::GiantBee => pick_random(rng, &["Hive", "Nest", "Swarm", "Colony"]),
        _ => pick_random(rng, &["Lair", "Den", "Nest", "Cave"]),
    };

    // Use biome-specific adjective sometimes
    let adjective = if rng.gen_bool(0.4) {
        name_gen.biome_adjective(biome_category, rng)
    } else {
        pick_random(rng, &[
            "Dark", "Shadow", "Grim", "Dread", "Cursed",
            "Ancient", "Forgotten", "Hidden", "Foul", "Twisted",
        ]).to_string()
    };

    format!("The {} {}", adjective, location_type)
}

/// Generate territory tiles for a lair
fn generate_territory(
    cx: usize,
    cy: usize,
    radius: usize,
    width: usize,
    height: usize,
) -> Vec<(usize, usize)> {
    let mut tiles = Vec::new();
    let r = radius as i32;

    for dy in -r..=r {
        for dx in -r..=r {
            let dist_sq = dx * dx + dy * dy;
            if dist_sq <= r * r {
                let nx = (cx as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (cy as i32 + dy).clamp(0, height as i32 - 1) as usize;
                tiles.push((nx, ny));
            }
        }
    }

    tiles
}

/// Helper to pick a random element
fn pick_random<'a>(rng: &mut ChaCha8Rng, options: &[&'a str]) -> &'a str {
    options[rng.gen_range(0..options.len())]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_monster_generation() {
        let heightmap = Tilemap::new_with(64, 32, 100.0f32);
        let biomes = Tilemap::new_with(64, 32, ExtendedBiome::TemperateForest);
        let stress = Tilemap::new_with(64, 32, 0.0f32);

        let registry = generate_monster_lairs(&heightmap, &biomes, &stress, 42);

        assert!(!registry.lairs.is_empty(), "Should have lairs");

        for lair in registry.lairs.values() {
            println!("{}: {} at ({}, {})",
                lair.name,
                lair.species.name(),
                lair.x,
                lair.y
            );
        }
    }
}
