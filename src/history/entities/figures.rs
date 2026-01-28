//! Notable historical figures (rulers, heroes, villains).

use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::history::{FigureId, RaceId, FactionId, DynastyId, ArtifactId, EventId, EntityId};
use crate::history::time::Date;
use super::traits::{Personality, Skill, Ability, DeathCause};

/// A notable individual in world history.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Figure {
    pub id: FigureId,
    pub name: String,
    pub epithet: Option<String>,

    // Identity
    pub race_id: RaceId,
    pub faction: Option<FactionId>,
    pub birth_date: Date,
    pub death_date: Option<Date>,
    pub cause_of_death: Option<DeathCause>,

    // Family
    pub parents: (Option<FigureId>, Option<FigureId>),
    pub spouse: Option<FigureId>,
    pub children: Vec<FigureId>,
    pub dynasty: Option<DynastyId>,

    // Traits
    pub personality: Personality,
    pub skills: HashMap<Skill, u8>,
    pub abilities: Vec<Ability>,

    // Positions held
    pub titles: Vec<String>,

    // Possessions
    pub artifacts: Vec<ArtifactId>,

    // Relationships
    pub enemies: Vec<FigureId>,
    pub mentors: Vec<FigureId>,

    // History
    pub events: Vec<EventId>,
    pub kills: Vec<EntityId>,

    // Quests
    pub active_quest: Option<EventId>,
}

impl Figure {
    pub fn new(
        id: FigureId,
        name: String,
        race_id: RaceId,
        birth_date: Date,
        personality: Personality,
    ) -> Self {
        Self {
            id,
            name,
            epithet: None,
            race_id,
            faction: None,
            birth_date,
            death_date: None,
            cause_of_death: None,
            parents: (None, None),
            spouse: None,
            children: Vec::new(),
            dynasty: None,
            personality,
            skills: HashMap::new(),
            abilities: Vec::new(),
            titles: Vec::new(),
            artifacts: Vec::new(),
            enemies: Vec::new(),
            mentors: Vec::new(),
            events: Vec::new(),
            kills: Vec::new(),
            active_quest: None,
        }
    }

    /// Full display name with epithet.
    pub fn full_name(&self) -> String {
        match &self.epithet {
            Some(ep) => format!("{} {}", self.name, ep),
            None => self.name.clone(),
        }
    }

    pub fn is_alive(&self) -> bool {
        self.death_date.is_none()
    }

    /// Age in years at a given date.
    pub fn age_at(&self, date: &Date) -> u32 {
        date.year.saturating_sub(self.birth_date.year)
    }

    /// Kill this figure.
    pub fn kill(&mut self, date: Date, cause: DeathCause) {
        self.death_date = Some(date);
        self.cause_of_death = Some(cause);
    }

    /// Add a child.
    pub fn add_child(&mut self, child_id: FigureId) {
        if !self.children.contains(&child_id) {
            self.children.push(child_id);
        }
    }

    /// Get a skill level (0 if not learned).
    pub fn skill_level(&self, skill: Skill) -> u8 {
        self.skills.get(&skill).copied().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_figure_creation() {
        let fig = Figure::new(
            FigureId(0),
            "Aldric".to_string(),
            RaceId(0),
            Date::new(1, Season::Spring),
            Personality::default(),
        );
        assert!(fig.is_alive());
        assert_eq!(fig.full_name(), "Aldric");
    }

    #[test]
    fn test_figure_with_epithet() {
        let mut fig = Figure::new(
            FigureId(0), "Aldric".to_string(), RaceId(0),
            Date::new(1, Season::Spring), Personality::default(),
        );
        fig.epithet = Some("the Bold".to_string());
        assert_eq!(fig.full_name(), "Aldric the Bold");
    }

    #[test]
    fn test_figure_death() {
        let mut fig = Figure::new(
            FigureId(0), "Aldric".to_string(), RaceId(0),
            Date::new(1, Season::Spring), Personality::default(),
        );
        fig.kill(Date::new(50, Season::Autumn), DeathCause::Battle);
        assert!(!fig.is_alive());
        assert_eq!(fig.age_at(&Date::new(50, Season::Autumn)), 49);
    }
}
