//! Monuments and structures that encode history.

use serde::{Serialize, Deserialize};
use crate::history::{MonumentId, FigureId, FactionId, EventId, EntityId};
use crate::history::time::Date;
use crate::history::civilizations::economy::ResourceType;
use super::artifacts::Inscription;

/// Types of monuments.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MonumentType {
    Statue,
    Obelisk,
    Tomb,
    Pyramid,
    Temple,
    Castle,
    Wall,
    Tower,
    Bridge,
    Fountain,
    Memorial,
    Trophy,
    Altar,
}

/// Purpose of a monument.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MonumentPurpose {
    CommemorateVictory,
    HonorDead,
    ReligiousWorship,
    Defense,
    MarkTerritory,
    CelebratePeace,
    WarnOthers,
    ArtisticExpression,
}

/// A monument on the world map.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Monument {
    pub id: MonumentId,
    pub name: String,
    pub monument_type: MonumentType,
    pub location: (usize, usize),

    pub built_date: Date,
    pub builder: Option<FigureId>,
    pub commissioned_by: Option<FigureId>,
    pub faction: FactionId,
    pub construction_event: Option<EventId>,

    pub commemorates: Option<EventId>,
    pub honors: Vec<EntityId>,
    pub purpose: MonumentPurpose,

    pub materials: Vec<ResourceType>,
    pub inscriptions: Vec<Inscription>,

    pub intact: bool,
    pub destruction_date: Option<Date>,
    pub destruction_event: Option<EventId>,
}

impl Monument {
    pub fn new(
        id: MonumentId,
        name: String,
        monument_type: MonumentType,
        location: (usize, usize),
        faction: FactionId,
        built_date: Date,
        purpose: MonumentPurpose,
    ) -> Self {
        Self {
            id,
            name,
            monument_type,
            location,
            built_date,
            builder: None,
            commissioned_by: None,
            faction,
            construction_event: None,
            commemorates: None,
            honors: Vec::new(),
            purpose,
            materials: Vec::new(),
            inscriptions: Vec::new(),
            intact: true,
            destruction_date: None,
            destruction_event: None,
        }
    }

    pub fn destroy(&mut self, date: Date, event: Option<EventId>) {
        self.intact = false;
        self.destruction_date = Some(date);
        self.destruction_event = event;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_monument_lifecycle() {
        let mut mon = Monument::new(
            MonumentId(0), "Victory Obelisk".to_string(),
            MonumentType::Obelisk, (50, 30),
            FactionId(0), Date::new(100, Season::Summer),
            MonumentPurpose::CommemorateVictory,
        );
        assert!(mon.intact);
        mon.destroy(Date::new(200, Season::Winter), None);
        assert!(!mon.intact);
    }
}
