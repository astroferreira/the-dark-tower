//! Dynasty and lineage tracking.

use serde::{Serialize, Deserialize};
use crate::history::{DynastyId, FigureId, FactionId, SettlementId, ArtifactId, EventId};
use crate::history::time::Date;
use crate::history::civilizations::government::SuccessionLaw;

/// A dynasty / noble house.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Dynasty {
    pub id: DynastyId,
    pub name: String,
    pub founded: Date,
    pub founder: FigureId,
    pub current_head: Option<FigureId>,
    pub succession_law: SuccessionLaw,

    pub members: Vec<FigureId>,
    pub generations: u32,

    pub factions_ruled: Vec<FactionId>,
    pub ancestral_seats: Vec<SettlementId>,
    pub heirlooms: Vec<ArtifactId>,

    pub prestige: u32,
    pub scandals: Vec<EventId>,
}

impl Dynasty {
    pub fn new(
        id: DynastyId,
        name: String,
        founded: Date,
        founder: FigureId,
        succession_law: SuccessionLaw,
    ) -> Self {
        Self {
            id,
            name,
            founded,
            founder,
            current_head: Some(founder),
            succession_law,
            members: vec![founder],
            generations: 1,
            factions_ruled: Vec::new(),
            ancestral_seats: Vec::new(),
            heirlooms: Vec::new(),
            prestige: 10,
            scandals: Vec::new(),
        }
    }

    /// Add a member to the dynasty.
    pub fn add_member(&mut self, figure: FigureId) {
        if !self.members.contains(&figure) {
            self.members.push(figure);
        }
    }

    /// Total members count.
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Whether the dynasty has an active head.
    pub fn has_living_head(&self) -> bool {
        self.current_head.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_dynasty_creation() {
        let dynasty = Dynasty::new(
            DynastyId(0),
            "House Ironhelm".to_string(),
            Date::new(1, Season::Spring),
            FigureId(0),
            SuccessionLaw::Primogeniture,
        );
        assert_eq!(dynasty.member_count(), 1);
        assert!(dynasty.has_living_head());
        assert_eq!(dynasty.generations, 1);
    }
}
