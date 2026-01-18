//! Mood and happiness system for colonists
//!
//! Tracks emotional state and modifiers affecting colonist behavior.

use serde::{Deserialize, Serialize};

/// Types of mood modifiers
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MoodModifierType {
    // Positive
    WellFed,
    WellRested,
    SafeEnvironment,
    GoodShelter,
    SocialFulfillment,
    JobSatisfaction,
    RecentVictory,
    Marriage,
    ChildBorn,
    SkillImprovement,
    LeaderBonus,

    // Negative
    Hungry,
    Exhausted,
    InDanger,
    PoorShelter,
    Lonely,
    BadJob,
    RecentDefeat,
    DeathOfFriend,
    DeathOfFamily,
    Wounded,
    Sick,
    Overworked,
    UnrestPenalty,
}

impl MoodModifierType {
    /// Get the base value for this modifier
    pub fn base_value(&self) -> f32 {
        match self {
            // Positive
            MoodModifierType::WellFed => 0.1,
            MoodModifierType::WellRested => 0.05,
            MoodModifierType::SafeEnvironment => 0.1,
            MoodModifierType::GoodShelter => 0.05,
            MoodModifierType::SocialFulfillment => 0.1,
            MoodModifierType::JobSatisfaction => 0.1,
            MoodModifierType::RecentVictory => 0.15,
            MoodModifierType::Marriage => 0.2,
            MoodModifierType::ChildBorn => 0.15,
            MoodModifierType::SkillImprovement => 0.05,
            MoodModifierType::LeaderBonus => 0.1,

            // Negative
            MoodModifierType::Hungry => -0.2,
            MoodModifierType::Exhausted => -0.1,
            MoodModifierType::InDanger => -0.15,
            MoodModifierType::PoorShelter => -0.1,
            MoodModifierType::Lonely => -0.1,
            MoodModifierType::BadJob => -0.1,
            MoodModifierType::RecentDefeat => -0.2,
            MoodModifierType::DeathOfFriend => -0.15,
            MoodModifierType::DeathOfFamily => -0.3,
            MoodModifierType::Wounded => -0.2,
            MoodModifierType::Sick => -0.15,
            MoodModifierType::Overworked => -0.1,
            MoodModifierType::UnrestPenalty => -0.15,
        }
    }

    /// Get the duration in ticks (0 = indefinite, needs manual removal)
    pub fn duration(&self) -> u64 {
        match self {
            // Short-term (need-based, refreshed each tick)
            MoodModifierType::WellFed => 1,
            MoodModifierType::Hungry => 1,
            MoodModifierType::WellRested => 1,
            MoodModifierType::Exhausted => 1,
            MoodModifierType::SafeEnvironment => 1,
            MoodModifierType::InDanger => 1,
            MoodModifierType::GoodShelter => 1,
            MoodModifierType::PoorShelter => 1,
            MoodModifierType::Overworked => 1,

            // Medium-term (events)
            MoodModifierType::RecentVictory => 8,        // 2 years
            MoodModifierType::RecentDefeat => 8,
            MoodModifierType::SkillImprovement => 4,    // 1 year
            MoodModifierType::Wounded => 0,             // Until healed
            MoodModifierType::Sick => 0,                // Until cured

            // Long-term (social)
            MoodModifierType::SocialFulfillment => 4,
            MoodModifierType::Lonely => 4,
            MoodModifierType::JobSatisfaction => 4,
            MoodModifierType::BadJob => 4,
            MoodModifierType::Marriage => 20,           // 5 years
            MoodModifierType::ChildBorn => 12,          // 3 years
            MoodModifierType::DeathOfFriend => 16,      // 4 years
            MoodModifierType::DeathOfFamily => 24,      // 6 years
            MoodModifierType::LeaderBonus => 4,
            MoodModifierType::UnrestPenalty => 4,
        }
    }

    /// Get the display name
    pub fn name(&self) -> &'static str {
        match self {
            MoodModifierType::WellFed => "Well Fed",
            MoodModifierType::WellRested => "Well Rested",
            MoodModifierType::SafeEnvironment => "Safe",
            MoodModifierType::GoodShelter => "Good Shelter",
            MoodModifierType::SocialFulfillment => "Happy Socially",
            MoodModifierType::JobSatisfaction => "Enjoys Work",
            MoodModifierType::RecentVictory => "Recent Victory",
            MoodModifierType::Marriage => "Recently Married",
            MoodModifierType::ChildBorn => "New Parent",
            MoodModifierType::SkillImprovement => "Learned Something",
            MoodModifierType::LeaderBonus => "Good Leadership",
            MoodModifierType::Hungry => "Hungry",
            MoodModifierType::Exhausted => "Exhausted",
            MoodModifierType::InDanger => "In Danger",
            MoodModifierType::PoorShelter => "Poor Shelter",
            MoodModifierType::Lonely => "Lonely",
            MoodModifierType::BadJob => "Dislikes Job",
            MoodModifierType::RecentDefeat => "Recent Defeat",
            MoodModifierType::DeathOfFriend => "Mourning Friend",
            MoodModifierType::DeathOfFamily => "Mourning Family",
            MoodModifierType::Wounded => "Wounded",
            MoodModifierType::Sick => "Sick",
            MoodModifierType::Overworked => "Overworked",
            MoodModifierType::UnrestPenalty => "Social Unrest",
        }
    }
}

/// A single mood modifier instance
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoodModifier {
    pub modifier_type: MoodModifierType,
    pub value: f32,
    pub ticks_remaining: Option<u64>,
}

impl MoodModifier {
    pub fn new(modifier_type: MoodModifierType) -> Self {
        let duration = modifier_type.duration();
        MoodModifier {
            value: modifier_type.base_value(),
            ticks_remaining: if duration > 0 { Some(duration) } else { None },
            modifier_type,
        }
    }

    pub fn with_value(modifier_type: MoodModifierType, value: f32) -> Self {
        let duration = modifier_type.duration();
        MoodModifier {
            modifier_type,
            value,
            ticks_remaining: if duration > 0 { Some(duration) } else { None },
        }
    }

    /// Returns true if this modifier has expired
    pub fn tick(&mut self) -> bool {
        if let Some(ref mut remaining) = self.ticks_remaining {
            *remaining = remaining.saturating_sub(1);
            *remaining == 0
        } else {
            false
        }
    }

    pub fn is_expired(&self) -> bool {
        self.ticks_remaining == Some(0)
    }
}

/// Overall mood state for a colonist
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MoodState {
    /// Base mood (personality-based, 0.4 - 0.6)
    pub base_mood: f32,
    /// Current mood level (0.0 = miserable, 1.0 = ecstatic)
    pub current_mood: f32,
    /// Active modifiers
    pub modifiers: Vec<MoodModifier>,
}

impl MoodState {
    pub fn new(base_mood: f32) -> Self {
        MoodState {
            base_mood,
            current_mood: base_mood,
            modifiers: Vec::new(),
        }
    }

    /// Add a mood modifier
    pub fn add_modifier(&mut self, modifier_type: MoodModifierType) {
        // Check if this type already exists
        if let Some(existing) = self.modifiers.iter_mut()
            .find(|m| m.modifier_type == modifier_type)
        {
            // Refresh duration
            let duration = modifier_type.duration();
            existing.ticks_remaining = if duration > 0 { Some(duration) } else { None };
        } else {
            self.modifiers.push(MoodModifier::new(modifier_type));
        }
        self.recalculate();
    }

    /// Add a modifier with a specific value
    pub fn add_modifier_with_value(&mut self, modifier_type: MoodModifierType, value: f32) {
        // Remove existing of same type
        self.modifiers.retain(|m| m.modifier_type != modifier_type);
        self.modifiers.push(MoodModifier::with_value(modifier_type, value));
        self.recalculate();
    }

    /// Remove a modifier by type
    pub fn remove_modifier(&mut self, modifier_type: MoodModifierType) {
        self.modifiers.retain(|m| m.modifier_type != modifier_type);
        self.recalculate();
    }

    /// Update mood state each tick
    pub fn tick(&mut self) {
        // Tick all modifiers and remove expired ones
        for modifier in &mut self.modifiers {
            modifier.tick();
        }
        self.modifiers.retain(|m| !m.is_expired());

        self.recalculate();
    }

    /// Recalculate current mood from base + modifiers
    fn recalculate(&mut self) {
        let modifier_sum: f32 = self.modifiers.iter().map(|m| m.value).sum();
        self.current_mood = (self.base_mood + modifier_sum).clamp(0.0, 1.0);
    }

    /// Get work productivity modifier from mood
    /// Low mood = reduced productivity, high mood = bonus
    pub fn work_modifier(&self) -> f32 {
        // 0.0 mood = 0.7x productivity
        // 0.5 mood = 1.0x productivity
        // 1.0 mood = 1.2x productivity
        0.7 + (self.current_mood * 0.5)
    }

    /// Get combat modifier from mood
    pub fn combat_modifier(&self) -> f32 {
        // Mood affects combat less than work
        0.8 + (self.current_mood * 0.4)
    }

    /// Get social modifier from mood
    pub fn social_modifier(&self) -> f32 {
        // Mood strongly affects social interactions
        0.5 + (self.current_mood * 1.0)
    }

    /// Get mood description
    pub fn description(&self) -> &'static str {
        match self.current_mood {
            x if x >= 0.9 => "Ecstatic",
            x if x >= 0.7 => "Happy",
            x if x >= 0.5 => "Content",
            x if x >= 0.3 => "Unhappy",
            x if x >= 0.1 => "Miserable",
            _ => "Broken",
        }
    }

    /// Check if colonist might break (tantrum, etc.)
    pub fn might_break(&self) -> bool {
        self.current_mood < 0.1
    }

    /// Get list of active modifier names
    pub fn active_modifier_names(&self) -> Vec<&'static str> {
        self.modifiers.iter()
            .map(|m| m.modifier_type.name())
            .collect()
    }
}

impl Default for MoodState {
    fn default() -> Self {
        MoodState::new(0.5)
    }
}

/// Apply needs-based mood modifiers to a mood state
pub fn apply_needs_modifiers(
    mood: &mut MoodState,
    food_satisfaction: f32,
    shelter_satisfaction: f32,
    security_satisfaction: f32,
    has_social: bool,
) {
    // Food
    if food_satisfaction > 0.8 {
        mood.add_modifier(MoodModifierType::WellFed);
    } else if food_satisfaction < 0.3 {
        mood.add_modifier(MoodModifierType::Hungry);
    } else {
        mood.remove_modifier(MoodModifierType::WellFed);
        mood.remove_modifier(MoodModifierType::Hungry);
    }

    // Shelter
    if shelter_satisfaction > 0.7 {
        mood.add_modifier(MoodModifierType::GoodShelter);
    } else if shelter_satisfaction < 0.3 {
        mood.add_modifier(MoodModifierType::PoorShelter);
    } else {
        mood.remove_modifier(MoodModifierType::GoodShelter);
        mood.remove_modifier(MoodModifierType::PoorShelter);
    }

    // Security
    if security_satisfaction > 0.7 {
        mood.add_modifier(MoodModifierType::SafeEnvironment);
    } else if security_satisfaction < 0.3 {
        mood.add_modifier(MoodModifierType::InDanger);
    } else {
        mood.remove_modifier(MoodModifierType::SafeEnvironment);
        mood.remove_modifier(MoodModifierType::InDanger);
    }

    // Social
    if has_social {
        mood.add_modifier(MoodModifierType::SocialFulfillment);
        mood.remove_modifier(MoodModifierType::Lonely);
    } else {
        mood.remove_modifier(MoodModifierType::SocialFulfillment);
        mood.add_modifier(MoodModifierType::Lonely);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mood_modifier() {
        let modifier = MoodModifier::new(MoodModifierType::WellFed);
        assert!(modifier.value > 0.0);
        assert!(modifier.ticks_remaining.is_some());
    }

    #[test]
    fn test_mood_state() {
        let mut mood = MoodState::new(0.5);
        assert_eq!(mood.current_mood, 0.5);

        mood.add_modifier(MoodModifierType::WellFed);
        assert!(mood.current_mood > 0.5);

        mood.add_modifier(MoodModifierType::Hungry);
        // Hungry should override well fed effect
        mood.recalculate();
    }

    #[test]
    fn test_mood_work_modifier() {
        let sad = MoodState::new(0.1);
        let happy = MoodState::new(0.9);

        assert!(sad.work_modifier() < happy.work_modifier());
    }
}
