//! Faction (nation/kingdom/tribe) definition.

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::history::{
    FactionId, SettlementId, FigureId, DynastyId, RaceId, ReligionId,
    ArmyId, WarId, TradeRouteId, EventId,
};
use crate::history::time::Date;
use super::diplomacy::DiplomaticRelation;
use super::economy::ResourceType;
use super::government::SuccessionLaw;
use crate::history::entities::culture::GovernmentType;

/// A political faction in the world.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Faction {
    pub id: FactionId,
    pub name: String,
    pub race_id: RaceId,
    pub founded: Date,
    pub dissolved: Option<Date>,

    // Territory
    pub capital: Option<SettlementId>,
    pub settlements: Vec<SettlementId>,

    // Leadership
    pub government: GovernmentType,
    pub current_leader: Option<FigureId>,
    pub ruling_dynasty: Option<DynastyId>,
    pub succession_law: SuccessionLaw,

    // Economy
    pub resources: HashMap<ResourceType, u32>,
    pub trade_routes: Vec<TradeRouteId>,
    pub wealth: u32,

    // Religion
    pub state_religion: Option<ReligionId>,
    pub religious_tolerance: f32,

    // Military
    pub military_strength: u32,
    pub armies: Vec<ArmyId>,
    pub wars: Vec<WarId>,

    // Diplomacy
    pub relations: HashMap<FactionId, DiplomaticRelation>,

    // History
    pub events: Vec<EventId>,
    pub notable_figures: Vec<FigureId>,

    // Population (aggregate total across settlements)
    pub total_population: u32,
}

impl Faction {
    pub fn new(
        id: FactionId,
        name: String,
        race_id: RaceId,
        founded: Date,
        government: GovernmentType,
        succession_law: SuccessionLaw,
    ) -> Self {
        Self {
            id,
            name,
            race_id,
            founded,
            dissolved: None,
            capital: None,
            settlements: Vec::new(),
            government,
            current_leader: None,
            ruling_dynasty: None,
            succession_law,
            resources: HashMap::new(),
            trade_routes: Vec::new(),
            wealth: 100,
            state_religion: None,
            religious_tolerance: 0.5,
            military_strength: 0,
            armies: Vec::new(),
            wars: Vec::new(),
            relations: HashMap::new(),
            events: Vec::new(),
            notable_figures: Vec::new(),
            total_population: 0,
        }
    }

    /// Whether this faction still exists.
    pub fn is_active(&self) -> bool {
        self.dissolved.is_none()
    }

    /// Dissolve this faction.
    pub fn dissolve(&mut self, date: Date) {
        self.dissolved = Some(date);
    }

    /// Add a settlement to this faction.
    pub fn add_settlement(&mut self, settlement_id: SettlementId) {
        if !self.settlements.contains(&settlement_id) {
            self.settlements.push(settlement_id);
        }
        if self.capital.is_none() {
            self.capital = Some(settlement_id);
        }
    }

    /// Remove a settlement.
    pub fn remove_settlement(&mut self, settlement_id: SettlementId) {
        self.settlements.retain(|s| *s != settlement_id);
        if self.capital == Some(settlement_id) {
            self.capital = self.settlements.first().copied();
        }
    }

    /// Whether this faction is at war with another.
    pub fn is_at_war_with(&self, other: FactionId) -> bool {
        self.relations.get(&other)
            .map(|r| r.stance.is_at_war())
            .unwrap_or(false)
    }

    /// Get or insert a default relation for a faction.
    pub fn get_relation_mut(&mut self, other: FactionId, cultural_similarity: f32) -> &mut DiplomaticRelation {
        self.relations.entry(other)
            .or_insert_with(|| DiplomaticRelation::new(cultural_similarity))
    }

    /// Number of active wars.
    pub fn active_war_count(&self) -> usize {
        self.relations.values()
            .filter(|r| r.stance.is_at_war())
            .count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_faction_creation() {
        let faction = Faction::new(
            FactionId(0),
            "The Irondelve Dwarves".to_string(),
            RaceId(0),
            Date::new(1, Season::Spring),
            GovernmentType::Monarchy,
            SuccessionLaw::Primogeniture,
        );
        assert!(faction.is_active());
        assert_eq!(faction.settlements.len(), 0);
    }

    #[test]
    fn test_faction_settlements() {
        let mut faction = Faction::new(
            FactionId(0), "Test".to_string(), RaceId(0),
            Date::new(1, Season::Spring),
            GovernmentType::Monarchy, SuccessionLaw::Primogeniture,
        );
        faction.add_settlement(SettlementId(0));
        assert_eq!(faction.capital, Some(SettlementId(0)));
        faction.add_settlement(SettlementId(1));
        faction.remove_settlement(SettlementId(0));
        assert_eq!(faction.capital, Some(SettlementId(1)));
    }

    #[test]
    fn test_faction_dissolve() {
        let mut faction = Faction::new(
            FactionId(0), "Test".to_string(), RaceId(0),
            Date::new(1, Season::Spring),
            GovernmentType::Monarchy, SuccessionLaw::Primogeniture,
        );
        faction.dissolve(Date::new(100, Season::Autumn));
        assert!(!faction.is_active());
    }
}
