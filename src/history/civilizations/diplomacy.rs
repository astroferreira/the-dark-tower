//! Diplomatic relations between factions.

use serde::{Serialize, Deserialize};
use crate::history::{FactionId, WarId, TreatyId};

/// Diplomatic stance between two factions.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DiplomaticStance {
    War,
    Hostile,
    Neutral,
    Friendly,
    Allied,
    Vassal,
    Overlord,
}

impl DiplomaticStance {
    /// Whether factions at this stance can trade.
    pub fn allows_trade(&self) -> bool {
        matches!(self, DiplomaticStance::Neutral | DiplomaticStance::Friendly | DiplomaticStance::Allied)
    }

    /// Whether factions at this stance are at war.
    pub fn is_at_war(&self) -> bool {
        matches!(self, DiplomaticStance::War)
    }

    /// Stance from raw opinion value.
    pub fn from_opinion(opinion: i32) -> Self {
        if opinion <= -75 { DiplomaticStance::War }
        else if opinion <= -40 { DiplomaticStance::Hostile }
        else if opinion <= 20 { DiplomaticStance::Neutral }
        else if opinion <= 60 { DiplomaticStance::Friendly }
        else { DiplomaticStance::Allied }
    }
}

/// Complete diplomatic relation between two factions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiplomaticRelation {
    pub stance: DiplomaticStance,
    /// -100 (hate) to +100 (love).
    pub opinion: i32,
    pub treaties: Vec<TreatyId>,
    pub last_war: Option<WarId>,
    /// Total trade value between factions.
    pub trade_value: u32,
    /// 0.0 to 1.0 - how similar the cultures are.
    pub cultural_similarity: f32,
}

impl DiplomaticRelation {
    pub fn new(cultural_similarity: f32) -> Self {
        // Initial opinion biased by cultural similarity (-25 to +25)
        let base_opinion = ((cultural_similarity - 0.5) * 50.0) as i32;
        let stance = DiplomaticStance::from_opinion(base_opinion);
        Self {
            stance,
            opinion: base_opinion,
            treaties: Vec::new(),
            last_war: None,
            trade_value: 0,
            cultural_similarity,
        }
    }

    /// Adjust opinion and recalculate stance.
    pub fn adjust_opinion(&mut self, delta: i32) {
        self.opinion = (self.opinion + delta).clamp(-100, 100);
        // Don't auto-downgrade from vassal/overlord
        if !matches!(self.stance, DiplomaticStance::Vassal | DiplomaticStance::Overlord) {
            self.stance = DiplomaticStance::from_opinion(self.opinion);
        }
    }

    /// Drift opinion toward cultural baseline.
    pub fn drift_toward_baseline(&mut self) {
        let baseline = ((self.cultural_similarity - 0.5) * 50.0) as i32;
        if self.opinion < baseline {
            self.opinion += 1;
        } else if self.opinion > baseline {
            self.opinion -= 1;
        }
    }

    /// Force war state.
    pub fn declare_war(&mut self, war_id: WarId) {
        self.stance = DiplomaticStance::War;
        self.opinion = self.opinion.min(-50);
        self.last_war = Some(war_id);
    }

    /// End war, setting stance to hostile.
    pub fn make_peace(&mut self) {
        if self.stance == DiplomaticStance::War {
            self.stance = DiplomaticStance::Hostile;
            self.opinion = self.opinion.max(-60);
        }
    }
}

/// A formal treaty between factions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Treaty {
    pub id: TreatyId,
    pub signatories: Vec<FactionId>,
    pub treaty_type: TreatyType,
    pub signed: crate::history::time::Date,
    pub expires: Option<crate::history::time::Date>,
    pub broken: bool,
}

/// Types of treaties.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TreatyType {
    Peace,
    TradeAgreement,
    DefensiveAlliance,
    MilitaryAlliance,
    NonAggression,
    Vassalage,
    Marriage,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stance_from_opinion() {
        assert_eq!(DiplomaticStance::from_opinion(-80), DiplomaticStance::War);
        assert_eq!(DiplomaticStance::from_opinion(0), DiplomaticStance::Neutral);
        assert_eq!(DiplomaticStance::from_opinion(70), DiplomaticStance::Allied);
    }

    #[test]
    fn test_relation_cultural_similarity() {
        let similar = DiplomaticRelation::new(0.9);
        let different = DiplomaticRelation::new(0.1);
        assert!(similar.opinion > different.opinion);
    }

    #[test]
    fn test_war_and_peace() {
        let mut rel = DiplomaticRelation::new(0.5);
        rel.declare_war(WarId(0));
        assert!(rel.stance.is_at_war());
        rel.make_peace();
        assert!(!rel.stance.is_at_war());
    }
}
