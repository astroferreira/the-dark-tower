//! Religion definition and management.

use serde::{Serialize, Deserialize};
use crate::history::{ReligionId, DeityId, FigureId, FactionId, TempleId};
use crate::history::time::Date;

/// A religion practiced by one or more factions.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Religion {
    pub id: ReligionId,
    pub name: String,
    pub deities: Vec<DeityId>,
    pub origin_date: Date,
    pub founder: Option<FigureId>,

    pub religious_head: Option<FigureId>,
    pub temples: Vec<TempleId>,
    pub holy_sites: Vec<(usize, usize)>,

    pub doctrines: Vec<Doctrine>,
    pub heresies: Vec<ReligionId>,
    pub hostile_religions: Vec<ReligionId>,

    pub follower_factions: Vec<FactionId>,
    pub follower_count: u32,
}

impl Religion {
    pub fn new(
        id: ReligionId,
        name: String,
        deities: Vec<DeityId>,
        origin_date: Date,
        founder: Option<FigureId>,
    ) -> Self {
        Self {
            id,
            name,
            deities,
            origin_date,
            founder,
            religious_head: None,
            temples: Vec::new(),
            holy_sites: Vec::new(),
            doctrines: Vec::new(),
            heresies: Vec::new(),
            hostile_religions: Vec::new(),
            follower_factions: Vec::new(),
            follower_count: 0,
        }
    }

    /// Whether this religion is a monotheistic faith.
    pub fn is_monotheistic(&self) -> bool {
        self.deities.len() == 1
    }

    /// Add a faction as a follower.
    pub fn add_follower_faction(&mut self, faction: FactionId) {
        if !self.follower_factions.contains(&faction) {
            self.follower_factions.push(faction);
        }
    }

    /// Create a heresy (splinter faith).
    pub fn add_heresy(&mut self, heresy_id: ReligionId) {
        if !self.heresies.contains(&heresy_id) {
            self.heresies.push(heresy_id);
        }
    }

    /// Check if the religion has a specific doctrine.
    pub fn has_doctrine(&self, doctrine: Doctrine) -> bool {
        self.doctrines.contains(&doctrine)
    }

    /// War modifier based on doctrines. Returns a multiplier (1.0 = neutral).
    /// HolyWar: +50% war chance. Pacifism: -70% war chance.
    pub fn war_modifier(&self) -> f32 {
        let mut mult = 1.0;
        if self.has_doctrine(Doctrine::HolyWar) {
            mult *= 2.5;
        }
        if self.has_doctrine(Doctrine::Pacifism) {
            mult *= 0.15;
        }
        mult
    }

    /// Diplomacy modifier based on doctrines. Returns a multiplier (1.0 = neutral).
    /// Proselytizing: +30% diplomacy. Isolationism: -50% diplomacy.
    pub fn diplomacy_modifier(&self) -> f32 {
        let mut mult = 1.0;
        if self.has_doctrine(Doctrine::Proselytizing) {
            mult *= 1.3;
        }
        if self.has_doctrine(Doctrine::Isolationism) {
            mult *= 0.5;
        }
        mult
    }

    /// Monument building modifier based on doctrines.
    /// MonasticTradition: +50%. Asceticism: -40%.
    pub fn monument_modifier(&self) -> f32 {
        let mut mult = 1.0;
        if self.has_doctrine(Doctrine::MonasticTradition) {
            mult *= 1.5;
        }
        if self.has_doctrine(Doctrine::Asceticism) {
            mult *= 0.6;
        }
        mult
    }
}

/// Religious doctrines that define practices.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Doctrine {
    Pacifism,
    HolyWar,
    Asceticism,
    Indulgence,
    Proselytizing,
    Isolationism,
    AncestorVeneration,
    NatureWorship,
    SacrificeRequired,
    MagicForbidden,
    MagicEncouraged,
    MonasticTradition,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_religion_creation() {
        let religion = Religion::new(
            ReligionId(0),
            "The Faith of Iron".to_string(),
            vec![DeityId(0), DeityId(1)],
            Date::new(50, Season::Spring),
            Some(FigureId(0)),
        );
        assert!(!religion.is_monotheistic());
        assert_eq!(religion.follower_count, 0);
    }
}
