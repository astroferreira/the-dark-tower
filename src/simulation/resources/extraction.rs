//! Resource extraction from biomes

use crate::biomes::ExtendedBiome;
use crate::simulation::types::{ResourceType, Season};

/// Resources available from a biome
pub struct BiomeResources {
    pub primary: Vec<(ResourceType, f32)>,
    pub secondary: Vec<(ResourceType, f32)>,
    pub rare: Vec<(ResourceType, f32)>,
}

/// Get base resources available from a biome
pub fn biome_resources(biome: ExtendedBiome) -> BiomeResources {
    use ExtendedBiome::*;
    use ResourceType::*;

    match biome {
        // Forests - Wood, Food, Leather
        BorealForest | TemperateForest | TropicalForest | TropicalRainforest => BiomeResources {
            primary: vec![(Wood, 4.0), (Food, 2.0)],
            secondary: vec![(Leather, 1.5), (Water, 1.0)],
            rare: vec![],
        },

        AncientGrove | MushroomForest => BiomeResources {
            primary: vec![(Wood, 3.0), (Food, 2.5)],
            secondary: vec![(Leather, 1.0)],
            rare: vec![(Spices, 0.3)],
        },

        // Grasslands - Food, Leather
        TemperateGrassland | Savanna => BiomeResources {
            primary: vec![(Food, 4.0), (Leather, 2.0)],
            secondary: vec![(Cloth, 1.0), (Water, 0.5)],
            rare: vec![],
        },

        Tundra => BiomeResources {
            primary: vec![(Food, 1.5), (Leather, 2.5)],
            secondary: vec![(Stone, 1.0)],
            rare: vec![],
        },

        // Mountains - Stone, Metals
        SnowyPeaks | AlpineTundra | RazorPeaks => BiomeResources {
            primary: vec![(Stone, 4.0)],
            secondary: vec![(Iron, 1.5), (Copper, 1.0)],
            rare: vec![(Gold, 0.2), (Gems, 0.1)],
        },

        Foothills => BiomeResources {
            primary: vec![(Stone, 3.0), (Food, 1.5)],
            secondary: vec![(Copper, 1.0), (Wood, 1.0)],
            rare: vec![(Iron, 0.5)],
        },

        // Deserts - Stone, Salt
        Desert | SaltFlats | SingingDunes | GlassDesert => BiomeResources {
            primary: vec![(Stone, 2.0), (Salt, 2.0)],
            secondary: vec![],
            rare: vec![(Obsidian, 0.3)],
        },

        Oasis => BiomeResources {
            primary: vec![(Water, 5.0), (Food, 3.0)],
            secondary: vec![(Cloth, 1.0)],
            rare: vec![(Spices, 0.5)],
        },

        // Wetlands - Food, Clay
        Swamp | Marsh | Bog | MangroveSaltmarsh => BiomeResources {
            primary: vec![(Food, 2.0), (Water, 3.0)],
            secondary: vec![(Clay, 2.0), (Leather, 1.0)],
            rare: vec![],
        },

        // Volcanic - Stone, Obsidian
        VolcanicWasteland | Ashlands | LavaField | Caldera => BiomeResources {
            primary: vec![(Stone, 3.0)],
            secondary: vec![(Obsidian, 2.0)],
            rare: vec![(Iron, 1.0), (Gems, 0.2)],
        },

        ObsidianFields | BasaltColumns => BiomeResources {
            primary: vec![(Stone, 4.0), (Obsidian, 3.0)],
            secondary: vec![],
            rare: vec![(Gems, 0.3)],
        },

        Geysers | HotSprings | FumaroleField => BiomeResources {
            primary: vec![(Water, 3.0)],
            secondary: vec![(Stone, 1.0), (Salt, 1.0)],
            rare: vec![(Gems, 0.2)],
        },

        // Coastal - Food (fish), Salt
        CoastalWater | Lagoon => BiomeResources {
            primary: vec![(Food, 3.0), (Salt, 1.5)],
            secondary: vec![(Water, 1.0)],
            rare: vec![],
        },

        // Cave/Karst - Stone, Minerals
        CaveEntrance | Sinkhole | KarstPlains | Cenote => BiomeResources {
            primary: vec![(Stone, 3.0)],
            secondary: vec![(Water, 2.0), (Clay, 1.0)],
            rare: vec![(Gems, 0.5), (Gold, 0.2)],
        },

        // Crystal biomes
        CrystalForest | CrystalWasteland | CrystalDepths => BiomeResources {
            primary: vec![(Stone, 2.0)],
            secondary: vec![(Gems, 2.0)],
            rare: vec![(Gold, 0.3)],
        },

        // Rare/Fantasy biomes
        TitanBones | BoneFields => BiomeResources {
            primary: vec![(Stone, 2.0)],
            secondary: vec![(Leather, 1.0)], // Bone working
            rare: vec![(Gems, 0.4)],
        },

        CoralPlateau | CoralReef | PearlGardens => BiomeResources {
            primary: vec![(Food, 2.5)],
            secondary: vec![(Salt, 1.0)],
            rare: vec![(Gems, 0.5)],
        },

        // Default for other biomes
        _ => BiomeResources {
            primary: vec![(Food, 1.0), (Water, 0.5)],
            secondary: vec![(Stone, 0.5)],
            rare: vec![],
        },
    }
}

/// Extract resources from a biome for a tick
pub fn extract_resources(
    biome: ExtendedBiome,
    season: Season,
    efficiency: f32,
) -> Vec<(ResourceType, f32)> {
    let base = biome_resources(biome);
    let season_mult = season.food_modifier();

    let mut result = Vec::new();

    // Primary resources always extracted
    for (resource, amount) in base.primary {
        let final_amount = if resource == ResourceType::Food {
            amount * season_mult * efficiency
        } else {
            amount * efficiency
        };
        result.push((resource, final_amount));
    }

    // Secondary resources extracted at reduced rate
    for (resource, amount) in base.secondary {
        let final_amount = if resource == ResourceType::Food {
            amount * season_mult * efficiency * 0.7
        } else {
            amount * efficiency * 0.7
        };
        if final_amount > 0.1 {
            result.push((resource, final_amount));
        }
    }

    // Rare resources have chance to be extracted
    for (resource, amount) in base.rare {
        let final_amount = amount * efficiency * 0.5;
        if final_amount > 0.05 {
            result.push((resource, final_amount));
        }
    }

    result
}

/// Get a description of what resources a biome provides
pub fn biome_resource_description(biome: ExtendedBiome) -> String {
    let resources = biome_resources(biome);
    let primary: Vec<String> = resources.primary.iter().map(|(r, _)| format!("{:?}", r)).collect();
    let secondary: Vec<String> = resources.secondary.iter().map(|(r, _)| format!("{:?}", r)).collect();

    let mut desc = format!("Primary: {}", primary.join(", "));
    if !secondary.is_empty() {
        desc.push_str(&format!("; Secondary: {}", secondary.join(", ")));
    }
    desc
}
