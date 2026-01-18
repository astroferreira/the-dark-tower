//! Colonist relationships and social interactions
//!
//! Tracks relationships between colonists and handles social interactions.

use rand::Rng;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::simulation::colonists::types::{Colonist, ColonistId, ColonistRole, LifeStage, Gender};

/// Relationship type between two colonists
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RelationshipType {
    /// Parent-child relationship
    Parent,
    Child,
    /// Siblings
    Sibling,
    /// Married partners
    Spouse,
    /// Romantic but not married
    Lover,
    /// Close friend
    CloseFriend,
    /// Regular friend
    Friend,
    /// Acquaintance
    Acquaintance,
    /// Rival or competitor
    Rival,
    /// Enemy
    Enemy,
    /// Mentor-student relationship
    Mentor,
    Student,
}

impl RelationshipType {
    /// Get the base opinion modifier for this relationship type
    pub fn base_opinion_modifier(&self) -> i32 {
        match self {
            RelationshipType::Parent | RelationshipType::Child => 40,
            RelationshipType::Sibling => 25,
            RelationshipType::Spouse => 60,
            RelationshipType::Lover => 45,
            RelationshipType::CloseFriend => 35,
            RelationshipType::Friend => 20,
            RelationshipType::Acquaintance => 5,
            RelationshipType::Rival => -15,
            RelationshipType::Enemy => -40,
            RelationshipType::Mentor | RelationshipType::Student => 25,
        }
    }
}

/// A relationship between two colonists
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Relationship {
    /// The other colonist in this relationship
    pub other_id: ColonistId,
    /// Type of relationship
    pub relationship_type: RelationshipType,
    /// Current opinion/affection level (-100 to 100)
    pub opinion: i32,
    /// How well they know each other (0-100)
    pub familiarity: u32,
    /// Tick when they last interacted
    pub last_interaction_tick: u64,
    /// Notable memories about the relationship
    pub memories: Vec<RelationshipMemory>,
}

impl Relationship {
    pub fn new(other_id: ColonistId, relationship_type: RelationshipType, current_tick: u64) -> Self {
        Relationship {
            other_id,
            relationship_type,
            opinion: relationship_type.base_opinion_modifier(),
            familiarity: match relationship_type {
                RelationshipType::Parent | RelationshipType::Child | RelationshipType::Sibling => 80,
                RelationshipType::Spouse => 90,
                RelationshipType::CloseFriend => 60,
                RelationshipType::Friend => 40,
                _ => 10,
            },
            last_interaction_tick: current_tick,
            memories: Vec::new(),
        }
    }

    /// Add a memory to this relationship
    pub fn add_memory(&mut self, memory: RelationshipMemory) {
        self.memories.push(memory);
        // Keep only last 10 memories
        if self.memories.len() > 10 {
            self.memories.remove(0);
        }
    }

    /// Modify opinion
    pub fn modify_opinion(&mut self, delta: i32) {
        self.opinion = (self.opinion + delta).clamp(-100, 100);
    }

    /// Increase familiarity from interaction
    pub fn interact(&mut self, current_tick: u64) {
        self.familiarity = (self.familiarity + 1).min(100);
        self.last_interaction_tick = current_tick;
    }

    /// Decay relationship over time without interaction
    pub fn decay(&mut self, current_tick: u64) {
        let ticks_since_interaction = current_tick.saturating_sub(self.last_interaction_tick);
        // Decay after 20 ticks (5 years) without interaction
        if ticks_since_interaction > 20 && self.familiarity > 0 {
            self.familiarity = self.familiarity.saturating_sub(1);
        }
    }
}

/// A memory about a relationship
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RelationshipMemory {
    /// What happened
    pub event: MemoryEvent,
    /// When it happened
    pub tick: u64,
    /// Opinion impact
    pub opinion_impact: i32,
}

/// Types of memorable events in relationships
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MemoryEvent {
    FirstMeeting,
    SharedMeal,
    GoodConversation,
    Argument,
    HelpedInNeed,
    Betrayal,
    SharedVictory,
    SharedLoss,
    GaveGift,
    ReceivedGift,
    WorkedTogether,
    FoughtTogether,
    SavedLife,
    Marriage,
    ChildBirth,
    Death,
    Insult,
    Compliment,
    TeachingMoment,
}

impl MemoryEvent {
    /// Get the default opinion impact for this event
    pub fn default_impact(&self) -> i32 {
        match self {
            MemoryEvent::FirstMeeting => 5,
            MemoryEvent::SharedMeal => 3,
            MemoryEvent::GoodConversation => 5,
            MemoryEvent::Argument => -10,
            MemoryEvent::HelpedInNeed => 15,
            MemoryEvent::Betrayal => -40,
            MemoryEvent::SharedVictory => 10,
            MemoryEvent::SharedLoss => 5,
            MemoryEvent::GaveGift => 8,
            MemoryEvent::ReceivedGift => 10,
            MemoryEvent::WorkedTogether => 3,
            MemoryEvent::FoughtTogether => 12,
            MemoryEvent::SavedLife => 30,
            MemoryEvent::Marriage => 25,
            MemoryEvent::ChildBirth => 20,
            MemoryEvent::Death => -5,
            MemoryEvent::Insult => -8,
            MemoryEvent::Compliment => 5,
            MemoryEvent::TeachingMoment => 7,
        }
    }

    /// Get a description of this event
    pub fn description(&self) -> &'static str {
        match self {
            MemoryEvent::FirstMeeting => "first met",
            MemoryEvent::SharedMeal => "shared a meal",
            MemoryEvent::GoodConversation => "had a good conversation",
            MemoryEvent::Argument => "had an argument",
            MemoryEvent::HelpedInNeed => "helped in time of need",
            MemoryEvent::Betrayal => "was betrayed",
            MemoryEvent::SharedVictory => "celebrated victory together",
            MemoryEvent::SharedLoss => "mourned a loss together",
            MemoryEvent::GaveGift => "gave a gift",
            MemoryEvent::ReceivedGift => "received a gift",
            MemoryEvent::WorkedTogether => "worked together",
            MemoryEvent::FoughtTogether => "fought side by side",
            MemoryEvent::SavedLife => "saved their life",
            MemoryEvent::Marriage => "got married",
            MemoryEvent::ChildBirth => "welcomed a child",
            MemoryEvent::Death => "mourned their passing",
            MemoryEvent::Insult => "was insulted",
            MemoryEvent::Compliment => "received a compliment",
            MemoryEvent::TeachingMoment => "learned something valuable",
        }
    }
}

/// Manager for all relationships in a tribe
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RelationshipManager {
    /// Relationships indexed by colonist ID
    /// relationships[a] contains a's relationships to others
    pub relationships: HashMap<ColonistId, Vec<Relationship>>,
}

impl RelationshipManager {
    pub fn new() -> Self {
        RelationshipManager {
            relationships: HashMap::new(),
        }
    }

    /// Add or update a relationship
    pub fn add_relationship(
        &mut self,
        colonist_a: ColonistId,
        colonist_b: ColonistId,
        relationship_type: RelationshipType,
        current_tick: u64,
    ) {
        // Add A -> B relationship
        let relationships_a = self.relationships.entry(colonist_a).or_default();
        if !relationships_a.iter().any(|r| r.other_id == colonist_b) {
            relationships_a.push(Relationship::new(colonist_b, relationship_type, current_tick));
        }

        // Add B -> A relationship (reciprocal)
        let reciprocal_type = match relationship_type {
            RelationshipType::Parent => RelationshipType::Child,
            RelationshipType::Child => RelationshipType::Parent,
            RelationshipType::Mentor => RelationshipType::Student,
            RelationshipType::Student => RelationshipType::Mentor,
            other => other,
        };
        let relationships_b = self.relationships.entry(colonist_b).or_default();
        if !relationships_b.iter().any(|r| r.other_id == colonist_a) {
            relationships_b.push(Relationship::new(colonist_a, reciprocal_type, current_tick));
        }
    }

    /// Get relationship between two colonists
    pub fn get_relationship(&self, from: ColonistId, to: ColonistId) -> Option<&Relationship> {
        self.relationships
            .get(&from)
            .and_then(|rels| rels.iter().find(|r| r.other_id == to))
    }

    /// Get mutable relationship
    pub fn get_relationship_mut(&mut self, from: ColonistId, to: ColonistId) -> Option<&mut Relationship> {
        self.relationships
            .get_mut(&from)
            .and_then(|rels| rels.iter_mut().find(|r| r.other_id == to))
    }

    /// Record an interaction between two colonists
    pub fn record_interaction(
        &mut self,
        colonist_a: ColonistId,
        colonist_b: ColonistId,
        event: MemoryEvent,
        current_tick: u64,
    ) {
        let impact = event.default_impact();

        // Update A's relationship with B
        if let Some(rel) = self.get_relationship_mut(colonist_a, colonist_b) {
            rel.interact(current_tick);
            rel.modify_opinion(impact);
            rel.add_memory(RelationshipMemory {
                event: event.clone(),
                tick: current_tick,
                opinion_impact: impact,
            });
        } else {
            // Create new acquaintance relationship
            self.add_relationship(colonist_a, colonist_b, RelationshipType::Acquaintance, current_tick);
            if let Some(rel) = self.get_relationship_mut(colonist_a, colonist_b) {
                rel.add_memory(RelationshipMemory {
                    event: MemoryEvent::FirstMeeting,
                    tick: current_tick,
                    opinion_impact: 5,
                });
            }
        }

        // Update B's relationship with A
        if let Some(rel) = self.get_relationship_mut(colonist_b, colonist_a) {
            rel.interact(current_tick);
            rel.modify_opinion(impact);
        }
    }

    /// Upgrade relationship type based on familiarity and opinion
    pub fn check_relationship_upgrade(&mut self, colonist_a: ColonistId, colonist_b: ColonistId) {
        let should_upgrade = if let Some(rel) = self.get_relationship(colonist_a, colonist_b) {
            match rel.relationship_type {
                RelationshipType::Acquaintance => {
                    rel.familiarity >= 30 && rel.opinion >= 20
                }
                RelationshipType::Friend => {
                    rel.familiarity >= 60 && rel.opinion >= 40
                }
                _ => false,
            }
        } else {
            false
        };

        if should_upgrade {
            if let Some(rel) = self.get_relationship_mut(colonist_a, colonist_b) {
                rel.relationship_type = match rel.relationship_type {
                    RelationshipType::Acquaintance => RelationshipType::Friend,
                    RelationshipType::Friend => RelationshipType::CloseFriend,
                    other => other,
                };
            }
            // Update reciprocal
            if let Some(rel) = self.get_relationship_mut(colonist_b, colonist_a) {
                rel.relationship_type = match rel.relationship_type {
                    RelationshipType::Acquaintance => RelationshipType::Friend,
                    RelationshipType::Friend => RelationshipType::CloseFriend,
                    other => other,
                };
            }
        }
    }

    /// Process relationship decay for all colonists
    pub fn process_decay(&mut self, current_tick: u64) {
        for relationships in self.relationships.values_mut() {
            for rel in relationships.iter_mut() {
                rel.decay(current_tick);
            }
        }
    }

    /// Get all friends of a colonist
    pub fn get_friends(&self, colonist: ColonistId) -> Vec<ColonistId> {
        self.relationships
            .get(&colonist)
            .map(|rels| {
                rels.iter()
                    .filter(|r| {
                        matches!(
                            r.relationship_type,
                            RelationshipType::Friend
                                | RelationshipType::CloseFriend
                                | RelationshipType::Spouse
                                | RelationshipType::Sibling
                        ) && r.opinion > 0
                    })
                    .map(|r| r.other_id)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get social standing of a colonist (based on relationships)
    pub fn social_standing(&self, colonist: ColonistId) -> i32 {
        self.relationships
            .get(&colonist)
            .map(|rels| {
                rels.iter()
                    .map(|r| (r.opinion * r.familiarity as i32) / 100)
                    .sum()
            })
            .unwrap_or(0)
    }
}

/// Generate a random social interaction between nearby colonists
pub fn generate_social_interaction<R: Rng>(
    colonist_a: &Colonist,
    colonist_b: &Colonist,
    relationship_manager: &mut RelationshipManager,
    current_tick: u64,
    rng: &mut R,
) -> Option<MemoryEvent> {
    // Determine interaction type based on context and personalities
    let roll = rng.gen::<f32>();

    let event = if roll < 0.3 {
        MemoryEvent::GoodConversation
    } else if roll < 0.5 {
        MemoryEvent::SharedMeal
    } else if roll < 0.6 {
        // Check for potential conflict based on personalities
        let conflict_chance = (20.0 - colonist_a.attributes.charisma as f32
            + 20.0 - colonist_b.attributes.charisma as f32) / 100.0;
        if rng.gen::<f32>() < conflict_chance {
            MemoryEvent::Argument
        } else {
            MemoryEvent::GoodConversation
        }
    } else if roll < 0.7 {
        MemoryEvent::WorkedTogether
    } else if roll < 0.8 {
        MemoryEvent::Compliment
    } else if roll < 0.85 {
        // Elder teaching
        if colonist_a.life_stage == LifeStage::Elder || colonist_b.life_stage == LifeStage::Elder {
            MemoryEvent::TeachingMoment
        } else {
            MemoryEvent::GoodConversation
        }
    } else if roll < 0.9 {
        MemoryEvent::GaveGift
    } else {
        // Small chance for insult if low charisma
        if colonist_a.attributes.charisma < 8 && rng.gen::<f32>() < 0.3 {
            MemoryEvent::Insult
        } else {
            MemoryEvent::SharedMeal
        }
    };

    relationship_manager.record_interaction(colonist_a.id, colonist_b.id, event.clone(), current_tick);

    Some(event)
}

/// Check if two colonists can potentially become romantic partners
pub fn can_be_romantic(colonist_a: &Colonist, colonist_b: &Colonist) -> bool {
    // Both must be adults
    if colonist_a.life_stage != LifeStage::Adult || colonist_b.life_stage != LifeStage::Adult {
        return false;
    }
    // Neither can be married already
    if colonist_a.spouse.is_some() || colonist_b.spouse.is_some() {
        return false;
    }
    // Different genders (simplified for now)
    if colonist_a.gender == colonist_b.gender {
        return false;
    }
    // Not close family
    if colonist_a.parents.0 == Some(colonist_b.id)
        || colonist_a.parents.1 == Some(colonist_b.id)
        || colonist_b.parents.0 == Some(colonist_a.id)
        || colonist_b.parents.1 == Some(colonist_a.id)
    {
        return false;
    }

    true
}

/// Try to develop romance between two colonists
pub fn try_romance<R: Rng>(
    colonist_a: &mut Colonist,
    colonist_b: &mut Colonist,
    relationship_manager: &mut RelationshipManager,
    current_tick: u64,
    rng: &mut R,
) -> bool {
    if !can_be_romantic(colonist_a, colonist_b) {
        return false;
    }

    // Check existing relationship
    if let Some(rel) = relationship_manager.get_relationship(colonist_a.id, colonist_b.id) {
        // Need high opinion and familiarity
        if rel.opinion >= 50 && rel.familiarity >= 50 {
            // Romance roll - affected by charisma
            let romance_chance = (colonist_a.attributes.charisma as f32
                + colonist_b.attributes.charisma as f32) / 40.0 * 0.3;

            if rng.gen::<f32>() < romance_chance {
                // Upgrade to lovers
                if let Some(rel) = relationship_manager.get_relationship_mut(colonist_a.id, colonist_b.id) {
                    rel.relationship_type = RelationshipType::Lover;
                    rel.modify_opinion(20);
                }
                if let Some(rel) = relationship_manager.get_relationship_mut(colonist_b.id, colonist_a.id) {
                    rel.relationship_type = RelationshipType::Lover;
                    rel.modify_opinion(20);
                }
                return true;
            }
        }
    }

    false
}
