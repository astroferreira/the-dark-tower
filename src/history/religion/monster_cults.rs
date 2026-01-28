//! Monster worship cults.
//!
//! Cults form around legendary creatures, offering sacrifices
//! in exchange for protection or power.

use serde::{Serialize, Deserialize};
use crate::history::{CultId, LegendaryCreatureId, FigureId};
use crate::history::time::Date;
use crate::history::civilizations::economy::ResourceType;
use crate::history::creatures::anatomy::MagicAbility;

/// A cult that worships a legendary creature.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MonsterCult {
    pub id: CultId,
    pub worshipped_creature: LegendaryCreatureId,
    pub name: String,
    pub founded: Date,
    pub founder: Option<FigureId>,

    pub members: Vec<FigureId>,
    pub member_count: u32,
    pub secret: bool,

    pub sacrifices: bool,
    pub offerings: Vec<ResourceType>,
    pub granted_powers: Vec<MagicAbility>,

    pub headquarters: Option<(usize, usize)>,
    pub shrines: Vec<(usize, usize)>,
}

impl MonsterCult {
    pub fn new(
        id: CultId,
        creature_id: LegendaryCreatureId,
        name: String,
        founded: Date,
        secret: bool,
    ) -> Self {
        Self {
            id,
            worshipped_creature: creature_id,
            name,
            founded,
            founder: None,
            members: Vec::new(),
            member_count: 0,
            secret,
            sacrifices: true,
            offerings: vec![ResourceType::Food],
            granted_powers: Vec::new(),
            headquarters: None,
            shrines: Vec::new(),
        }
    }

    /// Whether the cult is still significant (has members).
    pub fn is_active(&self) -> bool {
        self.member_count > 0
    }

    /// Add a member figure.
    pub fn add_member(&mut self, figure: FigureId) {
        if !self.members.contains(&figure) {
            self.members.push(figure);
            self.member_count += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_cult_creation() {
        let mut cult = MonsterCult::new(
            CultId(0),
            LegendaryCreatureId(0),
            "Cult of the Devourer".to_string(),
            Date::new(50, Season::Autumn),
            true,
        );
        assert!(!cult.is_active());
        cult.add_member(FigureId(0));
        assert!(cult.is_active());
        assert!(cult.secret);
    }
}
