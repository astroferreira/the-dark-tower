//! Event type definitions and causality.

use serde::{Serialize, Deserialize};
use crate::history::{EventId, FactionId, ArtifactId, MonumentId, EntityId, FigureId, SettlementId};
use crate::history::time::Date;
use crate::history::entities::traits::DeathCause;
use crate::history::civilizations::economy::ResourceType;

/// All possible event types in history.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum EventType {
    // Civilization lifecycle
    FactionFounded,
    FactionDestroyed,
    SettlementFounded,
    SettlementDestroyed,
    SettlementGrew,

    // Diplomacy
    TreatySigned,
    TreatyBroken,
    AllianceFormed,
    AllianceBroken,
    TradeRouteEstablished,

    // Conflict
    WarDeclared,
    WarEnded,
    BattleFought,
    SiegeBegun,
    SiegeEnded,
    Raid,
    Massacre,

    // Politics
    RulerCrowned,
    RulerDeposed,
    SuccessionCrisis,
    Rebellion,
    Coup,
    Assassination,

    // Religion
    ReligionFounded,
    Miracle,
    HolyWarDeclared,
    TempleBuilt,
    TempleProfaned,
    CultFormed,

    // Monsters
    CreatureAppeared,
    CreatureSlain,
    MonsterRaid,
    LairEstablished,
    LairDestroyed,
    PopulationMigrated,

    // Notable figures
    HeroBorn,
    HeroDied,
    QuestBegun,
    QuestCompleted,
    MasterworkCreated,

    // Artifacts
    ArtifactCreated,
    ArtifactLost,
    ArtifactFound,
    ArtifactDestroyed,

    // Monuments
    MonumentBuilt,
    MonumentDestroyed,

    // Natural disasters (terrain-triggered, not terrain-modifying)
    VolcanoErupted,
    Earthquake,
    Flood,
    Drought,
    Plague,
    MagicalCatastrophe,

    // Magic
    SpellInvented,
    MagicalExperiment,
    CurseApplied,
    CurseLifted,

    /// Catch-all for data-driven event types not yet mapped.
    Other,
}

impl EventType {
    /// Whether this event is significant enough to potentially define an era.
    pub fn is_major(&self) -> bool {
        matches!(self,
            EventType::FactionFounded | EventType::FactionDestroyed |
            EventType::WarDeclared | EventType::WarEnded |
            EventType::CreatureSlain | EventType::VolcanoErupted |
            EventType::Plague | EventType::MagicalCatastrophe |
            EventType::ReligionFounded | EventType::HolyWarDeclared
        )
    }
}

/// Outcome of an event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum EventOutcome {
    Success,
    Failure,
    Pyrrhic,     // Won but at great cost
    Stalemate,
    Ongoing,
    Unknown,
}

/// A consequence triggered by an event.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Consequence {
    PopulationChange(FactionId, i32),
    TerritoryGained(FactionId, Vec<(usize, usize)>),
    TerritoryLost(FactionId, Vec<(usize, usize)>),
    RelationChange(FactionId, FactionId, i32),
    ResourceChange(FactionId, ResourceType, i32),
    FigureDeath(FigureId, DeathCause),
    ArtifactTransfer(ArtifactId, EntityId, EntityId),
    SettlementDestroyed(SettlementId),
    SettlementFounded(SettlementId),
}

/// A historical event with full causality tracking.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Event {
    pub id: EventId,
    pub event_type: EventType,
    pub date: Date,

    // Location
    pub location: Option<(usize, usize)>,

    // Participants
    pub primary_participants: Vec<EntityId>,
    pub factions_involved: Vec<FactionId>,

    // Causality
    pub causes: Vec<EventId>,
    pub triggered_by: Option<EventId>,
    pub consequences: Vec<Consequence>,
    pub triggered_events: Vec<EventId>,

    // Results
    pub outcome: EventOutcome,
    pub artifacts_created: Vec<ArtifactId>,
    pub monuments_created: Vec<MonumentId>,

    // Description
    pub title: String,
    pub description: String,
    pub is_major: bool,
}

impl Event {
    pub fn new(
        id: EventId,
        event_type: EventType,
        date: Date,
        title: String,
        description: String,
    ) -> Self {
        let is_major = event_type.is_major();
        Self {
            id,
            event_type,
            date,
            location: None,
            primary_participants: Vec::new(),
            factions_involved: Vec::new(),
            causes: Vec::new(),
            triggered_by: None,
            consequences: Vec::new(),
            triggered_events: Vec::new(),
            outcome: EventOutcome::Success,
            artifacts_created: Vec::new(),
            monuments_created: Vec::new(),
            title,
            description,
            is_major,
        }
    }

    /// Set the location of this event.
    pub fn at_location(mut self, x: usize, y: usize) -> Self {
        self.location = Some((x, y));
        self
    }

    /// Add a primary participant.
    pub fn with_participant(mut self, entity: EntityId) -> Self {
        self.primary_participants.push(entity);
        self
    }

    /// Add an involved faction.
    pub fn with_faction(mut self, faction: FactionId) -> Self {
        if !self.factions_involved.contains(&faction) {
            self.factions_involved.push(faction);
        }
        self
    }

    /// Set the cause of this event.
    pub fn caused_by(mut self, event: EventId) -> Self {
        self.triggered_by = Some(event);
        if !self.causes.contains(&event) {
            self.causes.push(event);
        }
        self
    }

    /// Add a consequence.
    pub fn with_consequence(mut self, consequence: Consequence) -> Self {
        self.consequences.push(consequence);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_event_creation() {
        let event = Event::new(
            EventId(0),
            EventType::WarDeclared,
            Date::new(100, Season::Spring),
            "The War of Iron".to_string(),
            "War erupted between the Irondelve and Bitterforge dwarves.".to_string(),
        )
        .at_location(50, 30)
        .with_faction(FactionId(0))
        .with_faction(FactionId(1));

        assert!(event.is_major);
        assert_eq!(event.factions_involved.len(), 2);
        assert_eq!(event.location, Some((50, 30)));
    }

    #[test]
    fn test_event_causality() {
        let cause = Event::new(
            EventId(0), EventType::Assassination,
            Date::new(200, Season::Summer),
            "Assassination of King Aldric".to_string(),
            "King Aldric was poisoned.".to_string(),
        );

        let effect = Event::new(
            EventId(1), EventType::WarDeclared,
            Date::new(200, Season::Autumn),
            "War of Succession".to_string(),
            "War broke out after the king's death.".to_string(),
        ).caused_by(cause.id);

        assert_eq!(effect.triggered_by, Some(EventId(0)));
        assert!(effect.causes.contains(&EventId(0)));
    }
}
