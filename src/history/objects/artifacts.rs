//! Named artifacts with ownership history and inscriptions.

use serde::{Serialize, Deserialize};
use crate::history::{ArtifactId, FigureId, EventId, EntityId};
use crate::history::time::Date;

/// Type of artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ArtifactType {
    Weapon,
    Armor,
    Crown,
    Ring,
    Amulet,
    Staff,
    Book,
    Goblet,
    Instrument,
    Relic,
}

/// Quality tier of an artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ArtifactQuality {
    Fine,
    Superior,
    Masterwork,
    Legendary,
    Divine,
}

/// How an artifact was acquired.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum AcquisitionMethod {
    Created,
    Inherited,
    Gifted,
    Stolen,
    Looted,
    Found,
    Purchased,
    Won,
}

/// An inscription on an artifact or monument.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Inscription {
    pub text: String,
    pub translation: String,
    pub refers_to: Vec<EntityId>,
    pub date_inscribed: Date,
}

/// A named historical artifact.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Artifact {
    pub id: ArtifactId,
    pub name: String,
    pub item_type: ArtifactType,
    pub description: String,

    // Creation
    pub creation_date: Date,
    pub creator: Option<FigureId>,
    pub creation_event: Option<EventId>,
    pub creation_location: Option<(usize, usize)>,

    // Properties
    pub quality: ArtifactQuality,
    pub inscriptions: Vec<Inscription>,

    // History
    pub owner_history: Vec<(EntityId, Date, Option<Date>, AcquisitionMethod)>,
    pub current_owner: Option<EntityId>,
    pub current_location: Option<(usize, usize)>,
    pub lost: bool,
    pub destroyed: bool,

    // Value
    pub monetary_value: u32,
    pub historical_importance: u32,

    // Events
    pub involved_in: Vec<EventId>,
}

impl Artifact {
    pub fn new(
        id: ArtifactId,
        name: String,
        item_type: ArtifactType,
        quality: ArtifactQuality,
        creation_date: Date,
        creator: Option<FigureId>,
    ) -> Self {
        let value = match quality {
            ArtifactQuality::Fine => 50,
            ArtifactQuality::Superior => 100,
            ArtifactQuality::Masterwork => 250,
            ArtifactQuality::Legendary => 500,
            ArtifactQuality::Divine => 1000,
        };

        Self {
            id,
            name,
            item_type,
            description: String::new(),
            creation_date,
            creator,
            creation_event: None,
            creation_location: None,
            quality,
            inscriptions: Vec::new(),
            owner_history: Vec::new(),
            current_owner: None,
            current_location: None,
            lost: false,
            destroyed: false,
            monetary_value: value,
            historical_importance: 0,
            involved_in: Vec::new(),
        }
    }

    /// Transfer ownership to a new entity.
    pub fn transfer_to(&mut self, new_owner: EntityId, date: Date, method: AcquisitionMethod) {
        // Close current ownership
        if let Some(current) = &self.current_owner {
            if let Some(last) = self.owner_history.last_mut() {
                last.2 = Some(date);
            }
        }
        self.owner_history.push((new_owner.clone(), date, None, method));
        self.current_owner = Some(new_owner);
        self.lost = false;
    }

    /// Mark as lost.
    pub fn lose(&mut self, date: Date) {
        if let Some(last) = self.owner_history.last_mut() {
            last.2 = Some(date);
        }
        self.current_owner = None;
        self.lost = true;
    }

    /// Mark as destroyed.
    pub fn destroy(&mut self, date: Date) {
        if let Some(last) = self.owner_history.last_mut() {
            last.2 = Some(date);
        }
        self.current_owner = None;
        self.destroyed = true;
    }

    pub fn is_available(&self) -> bool {
        !self.destroyed && !self.lost
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_artifact_creation() {
        let art = Artifact::new(
            ArtifactId(0), "Grimjaw's Wrath".to_string(),
            ArtifactType::Weapon, ArtifactQuality::Legendary,
            Date::new(87, Season::Autumn), Some(FigureId(0)),
        );
        assert!(art.is_available());
        assert_eq!(art.monetary_value, 500);
    }

    #[test]
    fn test_artifact_ownership() {
        let mut art = Artifact::new(
            ArtifactId(0), "Test Sword".to_string(),
            ArtifactType::Weapon, ArtifactQuality::Fine,
            Date::new(1, Season::Spring), None,
        );
        art.transfer_to(EntityId::Figure(FigureId(0)), Date::new(10, Season::Spring), AcquisitionMethod::Created);
        art.transfer_to(EntityId::Figure(FigureId(1)), Date::new(50, Season::Summer), AcquisitionMethod::Inherited);
        assert_eq!(art.owner_history.len(), 2);
        assert_eq!(art.current_owner, Some(EntityId::Figure(FigureId(1))));
    }
}
