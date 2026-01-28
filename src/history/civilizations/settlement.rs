//! Settlement types and definitions.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::history::{SettlementId, FactionId, MonumentId, TempleId, ArtifactId, EventId};
use crate::history::time::Date;
use super::economy::ResourceType;

/// Settlement types.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SettlementType {
    Capital,
    City,
    Town,
    Village,
    Fort,
    Outpost,
    Camp,
    Temple,
    Mine,
    Port,
}

impl SettlementType {
    /// Max population for this settlement type.
    pub fn population_cap(&self) -> u32 {
        match self {
            SettlementType::Capital => 100_000,
            SettlementType::City => 50_000,
            SettlementType::Town => 10_000,
            SettlementType::Village => 2_000,
            SettlementType::Fort => 1_000,
            SettlementType::Outpost => 200,
            SettlementType::Camp => 500,
            SettlementType::Temple => 500,
            SettlementType::Mine => 1_000,
            SettlementType::Port => 20_000,
        }
    }

    /// Minimum population to qualify as this type.
    pub fn min_population(&self) -> u32 {
        match self {
            SettlementType::Capital => 5_000,
            SettlementType::City => 5_000,
            SettlementType::Town => 1_000,
            SettlementType::Village => 50,
            _ => 10,
        }
    }
}

/// Wall defense level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WallLevel {
    None,
    Palisade,
    StoneWall,
    Fortified,
    Citadel,
}

/// Building types that can exist in a settlement.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BuildingType {
    Granary,
    Barracks,
    Market,
    Smithy,
    Library,
    Temple,
    Palace,
    Wall,
    Harbor,
    Mine,
    Farm,
    Workshop,
    MageTower,
    TavernInn,
}

/// A settlement on the world map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Settlement {
    pub id: SettlementId,
    pub name: String,
    pub settlement_type: SettlementType,
    pub location: (usize, usize),
    pub faction: FactionId,
    pub founded: Date,
    pub destroyed: Option<Date>,

    // Population
    pub population: u32,
    pub population_cap: u32,
    pub growth_rate: f32,

    // Infrastructure
    pub buildings: Vec<BuildingType>,
    pub walls: WallLevel,
    pub trade_hub: bool,

    // Economy
    pub local_resources: Vec<ResourceType>,
    pub production: HashMap<ResourceType, f32>,
    pub trade_connections: Vec<SettlementId>,

    // Culture
    pub monuments: Vec<MonumentId>,
    pub temples: Vec<TempleId>,
    pub artifacts_present: Vec<ArtifactId>,

    // History
    pub events: Vec<EventId>,
}

impl Settlement {
    pub fn new(
        id: SettlementId,
        name: String,
        settlement_type: SettlementType,
        location: (usize, usize),
        faction: FactionId,
        founded: Date,
        local_resources: Vec<ResourceType>,
    ) -> Self {
        let population_cap = settlement_type.population_cap();
        let initial_pop = match settlement_type {
            SettlementType::Capital => 500,   // Was 5000
            SettlementType::City => 300,      // Was 3000
            SettlementType::Town => 50,       // Was 500
            SettlementType::Village => 10,    // Was 100
            _ => 5,                           // Was 50
        };
        Self {
            id,
            name,
            settlement_type,
            location,
            faction,
            founded,
            destroyed: None,
            population: initial_pop,
            population_cap,
            growth_rate: 0.005,  // 0.5% per year (was 2%)
            buildings: Vec::new(),
            walls: WallLevel::None,
            trade_hub: false,
            local_resources,
            production: HashMap::new(),
            trade_connections: Vec::new(),
            monuments: Vec::new(),
            temples: Vec::new(),
            artifacts_present: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn is_destroyed(&self) -> bool {
        self.destroyed.is_some()
    }

    /// Grow population by one season's worth.
    pub fn grow_population(&mut self) {
        if self.is_destroyed() { return; }
        let growth = (self.population as f32 * self.growth_rate / 4.0).ceil() as u32;
        self.population = (self.population + growth.max(1)).min(self.population_cap);
    }

    /// Upgrade settlement type if population warrants it.
    pub fn check_upgrade(&mut self) {
        if self.population >= 5000 && self.settlement_type == SettlementType::Town {
            self.settlement_type = SettlementType::City;
            self.population_cap = SettlementType::City.population_cap();
        } else if self.population >= 1000 && self.settlement_type == SettlementType::Village {
            self.settlement_type = SettlementType::Town;
            self.population_cap = SettlementType::Town.population_cap();
        }
    }

    /// Defensive strength (walls + garrison).
    pub fn defense_strength(&self) -> u32 {
        let wall_bonus = match self.walls {
            WallLevel::None => 0,
            WallLevel::Palisade => 100,
            WallLevel::StoneWall => 500,
            WallLevel::Fortified => 1000,
            WallLevel::Citadel => 2000,
        };
        let garrison = self.population / 10; // 10% can fight
        wall_bonus + garrison
    }

    /// Destroy this settlement.
    pub fn destroy(&mut self, date: Date) {
        self.destroyed = Some(date);
        self.population = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_settlement_growth() {
        let mut s = Settlement::new(
            SettlementId(0), "Testville".to_string(),
            SettlementType::Village, (0, 0), FactionId(0),
            Date::new(1, Season::Spring),
            vec![ResourceType::Food],
        );
        let initial = s.population;
        s.grow_population();
        assert!(s.population > initial);
    }

    #[test]
    fn test_settlement_upgrade() {
        let mut s = Settlement::new(
            SettlementId(0), "Testville".to_string(),
            SettlementType::Village, (0, 0), FactionId(0),
            Date::new(1, Season::Spring),
            vec![ResourceType::Food],
        );
        s.population = 1200;
        s.check_upgrade();
        assert_eq!(s.settlement_type, SettlementType::Town);
    }

    #[test]
    fn test_settlement_defense() {
        let mut s = Settlement::new(
            SettlementId(0), "Fort Test".to_string(),
            SettlementType::City, (0, 0), FactionId(0),
            Date::new(1, Season::Spring),
            vec![],
        );
        s.population = 5000;
        s.walls = WallLevel::Fortified;
        assert!(s.defense_strength() > 1000);
    }
}
