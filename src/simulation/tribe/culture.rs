//! Tribe culture system - extends CulturalLens from lore system

use rand::Rng;
use serde::{Deserialize, Serialize};
use crate::biomes::ExtendedBiome;
use crate::lore::types::CulturalLens;
use crate::simulation::types::ResourceType;

/// Extended culture for tribes, building on the lore system's CulturalLens
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TribeCulture {
    /// Base cultural lens from lore system
    pub lens: CulturalLens,
    /// Tribe's preferred biomes for settlement
    pub preferred_biomes: Vec<String>,
    /// Valued resources (tribe will prioritize these)
    pub valued_resources: Vec<ResourceType>,
    /// Aggression level (0.0 = peaceful, 1.0 = warlike)
    pub aggression: f32,
    /// Trade willingness (0.0 = isolationist, 1.0 = mercantile)
    pub trade_affinity: f32,
    /// Expansion drive (0.0 = sedentary, 1.0 = expansionist)
    pub expansion_drive: f32,
    /// Research priority (0.0 = traditional, 1.0 = innovative)
    pub research_priority: f32,
}

impl TribeCulture {
    /// Create a culture from a biome (influences cultural lens choice)
    pub fn from_biome<R: Rng>(biome: ExtendedBiome, rng: &mut R) -> Self {
        let lens = Self::lens_for_biome(biome, rng);
        let preferred_biomes = Self::biomes_for_lens(&lens);
        let valued_resources = Self::resources_for_lens(&lens);

        // Randomize personality traits based on lens
        let (base_aggr, base_trade, base_expan, base_research) = match &lens {
            CulturalLens::Highland { .. } => (0.4, 0.3, 0.3, 0.4),
            CulturalLens::Maritime { .. } => (0.3, 0.8, 0.5, 0.5),
            CulturalLens::Desert { .. } => (0.5, 0.6, 0.6, 0.3),
            CulturalLens::Sylvan { .. } => (0.2, 0.4, 0.2, 0.5),
            CulturalLens::Steppe { .. } => (0.7, 0.5, 0.8, 0.3),
            CulturalLens::Subterranean { .. } => (0.4, 0.3, 0.3, 0.6),
        };

        // Add randomness
        let variance: f32 = 0.2;
        let aggression = (base_aggr + rng.gen_range(-variance..variance)).clamp(0.0, 1.0);
        let trade_affinity = (base_trade + rng.gen_range(-variance..variance)).clamp(0.0, 1.0);
        let expansion_drive = (base_expan + rng.gen_range(-variance..variance)).clamp(0.0, 1.0);
        let research_priority = (base_research + rng.gen_range(-variance..variance)).clamp(0.0, 1.0);

        TribeCulture {
            lens,
            preferred_biomes,
            valued_resources,
            aggression,
            trade_affinity,
            expansion_drive,
            research_priority,
        }
    }

    /// Determine appropriate cultural lens for a biome
    fn lens_for_biome<R: Rng>(biome: ExtendedBiome, rng: &mut R) -> CulturalLens {
        use crate::lore::types::Direction;

        match biome {
            // Mountain/Alpine biomes -> Highland
            ExtendedBiome::SnowyPeaks
            | ExtendedBiome::AlpineTundra
            | ExtendedBiome::Foothills
            | ExtendedBiome::RazorPeaks => {
                let directions = Direction::all();
                CulturalLens::Highland {
                    sacred_direction: directions[rng.gen_range(0..directions.len())],
                    ancestor_worship: rng.gen_bool(0.6),
                }
            }

            // Coastal/Water biomes -> Maritime
            ExtendedBiome::CoastalWater
            | ExtendedBiome::Lagoon
            | ExtendedBiome::Ocean
            | ExtendedBiome::CoralReef => {
                let sea_names = ["Thalassa", "Aegir", "Poseidon", "Varuna", "Njord", "Mazu"];
                CulturalLens::Maritime {
                    sea_deity_name: sea_names[rng.gen_range(0..sea_names.len())].to_string(),
                    fears_deep_water: rng.gen_bool(0.3),
                }
            }

            // Desert biomes -> Desert
            ExtendedBiome::Desert
            | ExtendedBiome::SaltFlats
            | ExtendedBiome::SingingDunes
            | ExtendedBiome::GlassDesert => CulturalLens::Desert {
                follows_stars: rng.gen_bool(0.7),
                water_sacred: rng.gen_bool(0.9),
            },

            // Forest biomes -> Sylvan
            ExtendedBiome::BorealForest
            | ExtendedBiome::TemperateForest
            | ExtendedBiome::TropicalForest
            | ExtendedBiome::TropicalRainforest
            | ExtendedBiome::AncientGrove
            | ExtendedBiome::MushroomForest => CulturalLens::Sylvan {
                tree_worship: rng.gen_bool(0.6),
                fears_open_sky: rng.gen_bool(0.4),
            },

            // Grassland biomes -> Steppe
            ExtendedBiome::TemperateGrassland
            | ExtendedBiome::Savanna
            | ExtendedBiome::Tundra => CulturalLens::Steppe {
                sky_worship: rng.gen_bool(0.6),
                values_movement: rng.gen_bool(0.8),
            },

            // Underground/Cave biomes -> Subterranean
            ExtendedBiome::CaveEntrance
            | ExtendedBiome::Sinkhole
            | ExtendedBiome::HollowEarth
            | ExtendedBiome::Cenote => CulturalLens::Subterranean {
                fears_sunlight: rng.gen_bool(0.4),
                crystal_worship: rng.gen_bool(0.5),
            },

            // Default: random based on common types
            _ => {
                let roll = rng.gen_range(0..6);
                match roll {
                    0 => CulturalLens::Highland {
                        sacred_direction: Direction::North,
                        ancestor_worship: true,
                    },
                    1 => CulturalLens::Maritime {
                        sea_deity_name: "Oceanus".to_string(),
                        fears_deep_water: false,
                    },
                    2 => CulturalLens::Desert {
                        follows_stars: true,
                        water_sacred: true,
                    },
                    3 => CulturalLens::Sylvan {
                        tree_worship: true,
                        fears_open_sky: false,
                    },
                    4 => CulturalLens::Steppe {
                        sky_worship: true,
                        values_movement: true,
                    },
                    _ => CulturalLens::Subterranean {
                        fears_sunlight: false,
                        crystal_worship: true,
                    },
                }
            }
        }
    }

    /// Get preferred biomes for a cultural lens
    fn biomes_for_lens(lens: &CulturalLens) -> Vec<String> {
        match lens {
            CulturalLens::Highland { .. } => vec![
                "SnowyPeaks".to_string(),
                "AlpineTundra".to_string(),
                "Foothills".to_string(),
            ],
            CulturalLens::Maritime { .. } => vec![
                "CoastalWater".to_string(),
                "Lagoon".to_string(),
                "TemperateGrassland".to_string(), // Coastal plains
            ],
            CulturalLens::Desert { .. } => vec![
                "Desert".to_string(),
                "SaltFlats".to_string(),
                "Oasis".to_string(),
            ],
            CulturalLens::Sylvan { .. } => vec![
                "TemperateForest".to_string(),
                "BorealForest".to_string(),
                "TropicalForest".to_string(),
            ],
            CulturalLens::Steppe { .. } => vec![
                "TemperateGrassland".to_string(),
                "Savanna".to_string(),
                "Tundra".to_string(),
            ],
            CulturalLens::Subterranean { .. } => vec![
                "CaveEntrance".to_string(),
                "KarstPlains".to_string(),
                "Foothills".to_string(),
            ],
        }
    }

    /// Get valued resources for a cultural lens
    fn resources_for_lens(lens: &CulturalLens) -> Vec<ResourceType> {
        match lens {
            CulturalLens::Highland { .. } => vec![
                ResourceType::Stone,
                ResourceType::Iron,
                ResourceType::Gems,
            ],
            CulturalLens::Maritime { .. } => vec![
                ResourceType::Food, // Fish
                ResourceType::Salt,
                ResourceType::Cloth,
            ],
            CulturalLens::Desert { .. } => vec![
                ResourceType::Water,
                ResourceType::Salt,
                ResourceType::Spices,
            ],
            CulturalLens::Sylvan { .. } => vec![
                ResourceType::Wood,
                ResourceType::Food,
                ResourceType::Leather,
            ],
            CulturalLens::Steppe { .. } => vec![
                ResourceType::Leather,
                ResourceType::Food,
                ResourceType::Weapons,
            ],
            CulturalLens::Subterranean { .. } => vec![
                ResourceType::Stone,
                ResourceType::Gems,
                ResourceType::Obsidian,
            ],
        }
    }

    /// Check if a biome is preferred by this culture
    pub fn prefers_biome(&self, biome: &ExtendedBiome) -> bool {
        let biome_str = format!("{:?}", biome);
        self.preferred_biomes.contains(&biome_str)
    }

    /// Get terrain preference score for expansion (1.0 = neutral, >1 = preferred)
    pub fn terrain_preference(&self, biome: ExtendedBiome, elevation: f32, is_water: bool) -> f32 {
        self.lens.terrain_preference(biome, elevation, is_water)
    }

    /// Check if culture values a resource
    pub fn values_resource(&self, resource: ResourceType) -> bool {
        self.valued_resources.contains(&resource)
    }

    /// Get culture name from lens
    pub fn name(&self) -> &'static str {
        self.lens.culture_name()
    }

    /// Will this culture consider attacking?
    pub fn will_consider_attack<R: Rng>(&self, relation: i8, rng: &mut R) -> bool {
        if relation > 20 {
            return false; // Won't attack friends
        }

        // Base chance from aggression, modified by relations
        let relation_factor = (-relation as f32 / 100.0).max(0.0);
        let attack_chance = self.aggression * 0.5 + relation_factor * 0.5;
        rng.gen::<f32>() < attack_chance
    }

    /// Will this culture consider trading?
    pub fn will_consider_trade<R: Rng>(&self, relation: i8, rng: &mut R) -> bool {
        if relation < -50 {
            return false; // Won't trade with enemies
        }

        let relation_bonus = (relation as f32 / 100.0 + 0.5).clamp(0.0, 1.0);
        let trade_chance = self.trade_affinity * 0.7 + relation_bonus * 0.3;
        rng.gen::<f32>() < trade_chance
    }

    /// Check if this culture is warlike (high aggression)
    pub fn is_warlike(&self) -> bool {
        self.aggression > 0.5
    }

    /// Check if this culture is peaceful (low aggression)
    pub fn is_peaceful(&self) -> bool {
        self.aggression < 0.3
    }

    /// Check if this culture is trade-oriented
    pub fn is_mercantile(&self) -> bool {
        self.trade_affinity > 0.6
    }
}

/// Generate a tribe name based on culture and biome
pub fn generate_tribe_name<R: Rng>(culture: &TribeCulture, biome: ExtendedBiome, rng: &mut R) -> String {
    let prefixes = match &culture.lens {
        CulturalLens::Highland { .. } => &["Stone", "Peak", "Eagle", "Thunder", "Iron", "Cloud"][..],
        CulturalLens::Maritime { .. } => &["Wave", "Tide", "Shell", "Coral", "Salt", "Storm"][..],
        CulturalLens::Desert { .. } => &["Sand", "Sun", "Wind", "Star", "Dune", "Oasis"][..],
        CulturalLens::Sylvan { .. } => &["Oak", "Fern", "Moss", "Root", "Leaf", "Grove"][..],
        CulturalLens::Steppe { .. } => &["Horse", "Wind", "Sky", "Hawk", "Grass", "Thunder"][..],
        CulturalLens::Subterranean { .. } => &["Deep", "Crystal", "Shadow", "Stone", "Cave", "Dark"][..],
    };

    let suffixes = match &culture.lens {
        CulturalLens::Highland { .. } => &["walkers", "dwellers", "clan", "tribe", "folk", "kin"][..],
        CulturalLens::Maritime { .. } => &["sailors", "fishers", "people", "clan", "folk", "voyagers"][..],
        CulturalLens::Desert { .. } => &["wanderers", "seekers", "tribe", "nomads", "people", "clan"][..],
        CulturalLens::Sylvan { .. } => &["keepers", "watchers", "clan", "folk", "children", "tribe"][..],
        CulturalLens::Steppe { .. } => &["riders", "hunters", "horde", "clan", "people", "tribe"][..],
        CulturalLens::Subterranean { .. } => &["dwellers", "delvers", "clan", "folk", "tribe", "miners"][..],
    };

    let prefix = prefixes[rng.gen_range(0..prefixes.len())];
    let suffix = suffixes[rng.gen_range(0..suffixes.len())];

    format!("{}{}", prefix, suffix)
}
