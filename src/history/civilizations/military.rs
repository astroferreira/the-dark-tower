//! Military systems: armies, wars, sieges.

use serde::{Serialize, Deserialize};
use crate::history::{ArmyId, WarId, FactionId, FigureId, SettlementId, EventId};
use crate::history::time::Date;

/// An army fielded by a faction.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Army {
    pub id: ArmyId,
    pub faction: FactionId,
    pub commander: Option<FigureId>,
    pub strength: u32,
    pub morale: f32,
    pub location: (usize, usize),
    pub formed: Date,
    pub disbanded: Option<Date>,
}

impl Army {
    pub fn new(
        id: ArmyId,
        faction: FactionId,
        strength: u32,
        location: (usize, usize),
        formed: Date,
    ) -> Self {
        Self {
            id,
            faction,
            commander: None,
            strength,
            morale: 0.7,
            location,
            formed,
            disbanded: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self.disbanded.is_none() && self.strength > 0
    }

    /// Apply casualties. Returns remaining strength.
    pub fn take_casualties(&mut self, losses: u32) -> u32 {
        self.strength = self.strength.saturating_sub(losses);
        if self.strength == 0 {
            self.morale = 0.0;
        } else {
            // Morale drops with casualties
            let loss_ratio = losses as f32 / (self.strength + losses) as f32;
            self.morale = (self.morale - loss_ratio * 0.5).max(0.0);
        }
        self.strength
    }

    pub fn disband(&mut self, date: Date) {
        self.disbanded = Some(date);
    }

    /// Effective combat power (strength * morale).
    pub fn combat_power(&self) -> f32 {
        self.strength as f32 * self.morale
    }
}

/// A war between factions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct War {
    pub id: WarId,
    pub name: String,
    pub aggressors: Vec<FactionId>,
    pub defenders: Vec<FactionId>,
    pub started: Date,
    pub ended: Option<Date>,
    pub cause: WarCause,
    pub declaration_event: Option<EventId>,
    pub battles: Vec<EventId>,
    pub sieges: Vec<EventId>,
    pub victor: Option<FactionId>,
    pub casualties: WarCasualties,
}

impl War {
    pub fn new(
        id: WarId,
        name: String,
        aggressor: FactionId,
        defender: FactionId,
        started: Date,
        cause: WarCause,
    ) -> Self {
        Self {
            id,
            name,
            aggressors: vec![aggressor],
            defenders: vec![defender],
            started,
            ended: None,
            cause,
            declaration_event: None,
            battles: Vec::new(),
            sieges: Vec::new(),
            victor: None,
            casualties: WarCasualties::default(),
        }
    }

    pub fn is_active(&self) -> bool {
        self.ended.is_none()
    }

    pub fn end(&mut self, date: Date, victor: Option<FactionId>) {
        self.ended = Some(date);
        self.victor = victor;
    }

    /// Duration in years (None if ongoing).
    pub fn duration_years(&self) -> Option<u32> {
        self.ended.map(|end| end.year.saturating_sub(self.started.year))
    }
}

/// A siege of a settlement during a war.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Siege {
    pub id: crate::history::SiegeId,
    pub war_id: WarId,
    pub attacker: FactionId,
    pub defender: FactionId,
    pub target: SettlementId,
    pub started: Date,
    pub ended: Option<Date>,
    pub attacker_strength: u32,
    pub defender_strength: u32,
    pub attrition_days: u32,
    pub successful: Option<bool>,
    pub begin_event: Option<EventId>,
}

impl Siege {
    pub fn new(
        id: crate::history::SiegeId,
        war_id: WarId,
        attacker: FactionId,
        defender: FactionId,
        target: SettlementId,
        started: Date,
        attacker_strength: u32,
        defender_strength: u32,
    ) -> Self {
        Self {
            id,
            war_id,
            attacker,
            defender,
            target,
            started,
            ended: None,
            attacker_strength,
            defender_strength,
            attrition_days: 0,
            successful: None,
            begin_event: None,
        }
    }

    pub fn is_active(&self) -> bool {
        self.ended.is_none()
    }

    pub fn end(&mut self, date: Date, successful: bool) {
        self.ended = Some(date);
        self.successful = Some(successful);
    }

    /// Seasons elapsed since siege began.
    pub fn duration_seasons(&self, current: &Date) -> u32 {
        let years = current.year.saturating_sub(self.started.year);
        let season_diff = current.season as u32 - self.started.season as u32;
        years * 4 + season_diff
    }
}

/// Reason a war started.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WarCause {
    Territorial,
    Succession,
    Religious,
    Resource,
    Revenge,
    Conquest,
    Independence,
    HolyWar,
    DefensivePact,
}

/// Casualty tracking.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct WarCasualties {
    pub aggressor_losses: u32,
    pub defender_losses: u32,
    pub civilian_losses: u32,
    pub settlements_destroyed: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_army_casualties() {
        let mut army = Army::new(
            ArmyId(0), FactionId(0), 1000, (0, 0),
            Date::new(1, Season::Spring),
        );
        assert_eq!(army.combat_power(), 700.0);
        army.take_casualties(200);
        assert_eq!(army.strength, 800);
        assert!(army.morale < 0.7);
        assert!(army.is_active());
    }

    #[test]
    fn test_war_lifecycle() {
        let mut war = War::new(
            WarId(0), "War of the Iron Crown".to_string(),
            FactionId(0), FactionId(1),
            Date::new(100, Season::Spring),
            WarCause::Territorial,
        );
        assert!(war.is_active());
        war.end(Date::new(105, Season::Autumn), Some(FactionId(0)));
        assert!(!war.is_active());
        assert_eq!(war.duration_years(), Some(5));
    }
}
