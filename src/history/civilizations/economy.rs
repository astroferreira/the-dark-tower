//! Resource types and trade routes.

use serde::{Serialize, Deserialize};
use crate::biomes::ExtendedBiome;
use crate::history::{TradeRouteId, SettlementId};
use crate::history::time::Date;

/// Trade resource types tied to terrain/biomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ResourceType {
    // Basic
    Food,
    Wood,
    Stone,
    // Metals
    Iron,
    Copper,
    Gold,
    Silver,
    Mithril,
    Adamantine,
    // Gems
    Gems,
    Diamonds,
    Rubies,
    Emeralds,
    // Special
    Spices,
    Silk,
    Wine,
    Salt,
    Herbs,
    MagicalComponents,
    AncientRelics,
    // Monster-derived
    DragonScale,
    MonsterBones,
    Ichor,
}

impl ResourceType {
    /// Resources available from a given biome.
    pub fn from_biome(biome: ExtendedBiome) -> Vec<ResourceType> {
        let mut resources = vec![ResourceType::Food]; // Most biomes have food
        match biome {
            ExtendedBiome::TemperateForest | ExtendedBiome::BorealForest |
            ExtendedBiome::TropicalForest | ExtendedBiome::TemperateRainforest |
            ExtendedBiome::TropicalRainforest => {
                resources.extend_from_slice(&[ResourceType::Wood, ResourceType::Herbs]);
            }
            ExtendedBiome::SnowyPeaks | ExtendedBiome::AlpineTundra |
            ExtendedBiome::Foothills => {
                resources.extend_from_slice(&[ResourceType::Stone, ResourceType::Iron, ResourceType::Copper]);
            }
            ExtendedBiome::Desert | ExtendedBiome::SaltFlats => {
                resources.clear();
                resources.extend_from_slice(&[ResourceType::Salt, ResourceType::Stone]);
            }
            ExtendedBiome::Savanna | ExtendedBiome::TemperateGrassland => {
                resources.extend_from_slice(&[ResourceType::Food]); // Extra food
            }
            ExtendedBiome::Swamp | ExtendedBiome::Marsh | ExtendedBiome::Bog => {
                resources.extend_from_slice(&[ResourceType::Herbs]);
            }
            ExtendedBiome::CoastalWater | ExtendedBiome::Lagoon => {
                resources.extend_from_slice(&[ResourceType::Food, ResourceType::Salt]);
            }
            ExtendedBiome::VolcanicWasteland | ExtendedBiome::ObsidianFields => {
                resources.clear();
                resources.extend_from_slice(&[ResourceType::Stone, ResourceType::Gems]);
            }
            ExtendedBiome::CrystalForest | ExtendedBiome::CrystalWasteland => {
                resources.clear();
                resources.extend_from_slice(&[ResourceType::Gems, ResourceType::MagicalComponents]);
            }
            ExtendedBiome::LeyNexus | ExtendedBiome::EtherealMist => {
                resources.extend_from_slice(&[ResourceType::MagicalComponents]);
            }
            _ => {}
        }
        resources
    }

    /// Whether this is a luxury/high-value resource.
    pub fn is_luxury(&self) -> bool {
        matches!(self,
            ResourceType::Gold | ResourceType::Silver | ResourceType::Mithril |
            ResourceType::Adamantine | ResourceType::Gems | ResourceType::Diamonds |
            ResourceType::Rubies | ResourceType::Emeralds | ResourceType::Spices |
            ResourceType::Silk | ResourceType::Wine | ResourceType::MagicalComponents |
            ResourceType::AncientRelics | ResourceType::DragonScale
        )
    }

    /// Base trade value of this resource.
    pub fn base_value(&self) -> u32 {
        match self {
            ResourceType::Food | ResourceType::Wood | ResourceType::Stone => 1,
            ResourceType::Iron | ResourceType::Copper | ResourceType::Salt => 3,
            ResourceType::Herbs | ResourceType::Wine | ResourceType::Spices => 5,
            ResourceType::Gold | ResourceType::Silver => 8,
            ResourceType::Silk | ResourceType::Gems => 10,
            ResourceType::Diamonds | ResourceType::Rubies | ResourceType::Emeralds => 15,
            ResourceType::MagicalComponents => 12,
            ResourceType::AncientRelics => 20,
            ResourceType::Mithril => 25,
            ResourceType::Adamantine => 30,
            ResourceType::DragonScale | ResourceType::MonsterBones | ResourceType::Ichor => 15,
        }
    }
}

/// A trade route between two settlements.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TradeRoute {
    pub id: TradeRouteId,
    pub endpoints: (SettlementId, SettlementId),
    pub path: Vec<(usize, usize)>,
    pub established: Date,
    pub dissolved: Option<Date>,
    pub goods_traded: Vec<ResourceType>,
    pub value: u32,
    /// 0.0 = very dangerous, 1.0 = perfectly safe.
    pub safety: f32,
}

impl TradeRoute {
    pub fn new(
        id: TradeRouteId,
        from: SettlementId,
        to: SettlementId,
        established: Date,
        goods: Vec<ResourceType>,
    ) -> Self {
        let value = goods.iter().map(|g| g.base_value()).sum();
        Self {
            id,
            endpoints: (from, to),
            path: Vec::new(),
            established,
            dissolved: None,
            goods_traded: goods,
            value,
            safety: 0.8,
        }
    }

    pub fn is_active(&self) -> bool {
        self.dissolved.is_none()
    }

    pub fn dissolve(&mut self, date: Date) {
        self.dissolved = Some(date);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_biome_resources() {
        let forest_res = ResourceType::from_biome(ExtendedBiome::TemperateForest);
        assert!(forest_res.contains(&ResourceType::Wood));
        assert!(forest_res.contains(&ResourceType::Food));

        let desert_res = ResourceType::from_biome(ExtendedBiome::Desert);
        assert!(!desert_res.contains(&ResourceType::Food));
        assert!(desert_res.contains(&ResourceType::Salt));
    }

    #[test]
    fn test_resource_value() {
        assert!(ResourceType::Adamantine.base_value() > ResourceType::Iron.base_value());
        assert!(ResourceType::Gold.is_luxury());
        assert!(!ResourceType::Food.is_luxury());
    }
}
