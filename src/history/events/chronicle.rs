//! Chronicle: the complete event log with indexing.

use std::collections::BTreeMap;
use crate::history::det::HashMap;
use serde::{Serialize, Deserialize};
use crate::history::EventId;
use crate::history::time::Date;
use super::types::Event;

/// The chronicle of all events in world history.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Chronicle {
    /// All events in chronological order.
    pub events: Vec<Event>,
    /// Events indexed by date.
    pub by_date: BTreeMap<Date, Vec<EventId>>,
    /// Events indexed by location tile.
    pub by_location: HashMap<(usize, usize), Vec<EventId>>,
    /// Position of each event in `events` (not saved: rebuilt as events are recorded; lookups
    /// fall back to a scan for a loaded history).
    #[serde(skip)]
    by_id: HashMap<EventId, usize>,
    /// The latest event between two peoples (keyed lower id first), for finding causes.
    #[serde(skip)]
    last_pair: HashMap<(crate::history::FactionId, crate::history::FactionId), EventId>,
    /// The latest event each person, place, beast or people took part in.
    #[serde(skip)]
    last_entity: HashMap<crate::history::EntityId, EventId>,
}

impl Chronicle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new event.
    pub fn record(&mut self, mut event: Event) {
        if event.causes.is_empty() {
            if let Some(c) = self.infer_cause(&event) { event = event.caused_by(c); }
        }
        let id = event.id;
        let date = event.date;
        let location = event.location;

        // Index by date
        self.by_date.entry(date).or_default().push(id);

        // Index by location
        if let Some(loc) = location {
            self.by_location.entry(loc).or_default().push(id);
        }

        // Cause indexes: the latest event per pair of peoples and per participant.
        let f = &event.factions_involved;
        for i in 0..f.len() {
            for j in i + 1..f.len() {
                let key = if f[i] < f[j] { (f[i], f[j]) } else { (f[j], f[i]) };
                self.last_pair.insert(key, id);
            }
            self.last_entity.insert(crate::history::EntityId::Faction(f[i]), id);
        }
        for p in &event.primary_participants { self.last_entity.insert(p.clone(), id); }

        self.by_id.insert(id, self.events.len());
        self.events.push(event);
    }

    /// The cause of an event recorded without one, by a rule per kind, from what the chronicle
    /// already holds. Every rule names a real predecessor in the world, never a guess:
    /// - quarrels, war declarations, treaties, alliances and assassinations between two peoples
    ///   follow the last thing that passed between them (the grudge or friendship they act on);
    ///   a conversion follows the last dealing with the missionaries' people; a trade route
    ///   follows the peace it opens on (treaty, alliance, war's end, an earlier route);
    /// - a beast's raid follows its lair or its previous raid; a quest follows the last deed of
    ///   its target (the beast's raid, the treasure's loss);
    /// - a colony follows its mother town's last event; the land's answers to a town (forests
    ///   felled, game scarce, wolves in the ruins) follow that town's last event.
    /// Rulers crowned and other events with a known predecessor set their cause where they are made.
    fn infer_cause(&self, e: &Event) -> Option<EventId> {
        use super::types::EventType as T;
        use crate::history::EntityId as E;
        let pair = || {
            let f = &e.factions_involved;
            (f.len() >= 2).then(|| self.last_between(f[0], f[1])).flatten()
        };
        let participant = |pick: &dyn Fn(&E) -> bool| e.primary_participants.iter().filter(|p| pick(p)).find_map(|p| self.last_of(p.clone()));
        match e.event_type {
            T::Raid | T::WarDeclared | T::HolyWarDeclared | T::TreatySigned | T::TreatyBroken
            | T::AllianceFormed | T::AllianceBroken | T::Assassination => pair(),
            // A conversion follows the last dealing with the people whose missionaries brought it.
            T::Miracle if e.title.contains(" converts to ") => pair(),
            // Trade opens on peace: a treaty, an alliance, a war's end or an earlier route.
            T::TradeRouteEstablished => pair().filter(|&c| self.get(c).map_or(false, |ce| matches!(ce.event_type,
                T::TreatySigned | T::AllianceFormed | T::WarEnded | T::TradeRouteEstablished))),
            T::MonsterRaid => participant(&|p| matches!(p, E::LegendaryCreature(_))),
            T::QuestBegun => participant(&|p| matches!(p, E::LegendaryCreature(_) | E::Artifact(_))),
            T::SettlementFounded => {
                // The mother town is the second settlement named (the first is the new one).
                e.primary_participants.iter().filter(|p| matches!(p, E::Settlement(_))).nth(1).and_then(|p| self.last_of(p.clone()))
            }
            T::ForestCleared | T::GameScarce | T::WildlifeReturned => participant(&|p| matches!(p, E::Settlement(_))),
            _ => None,
        }
    }

    /// The latest event the two peoples were both part of.
    pub fn last_between(&self, a: crate::history::FactionId, b: crate::history::FactionId) -> Option<EventId> {
        self.last_pair.get(&if a < b { (a, b) } else { (b, a) }).copied()
    }

    /// The latest event a person, place, beast or people took part in.
    pub fn last_of(&self, e: crate::history::EntityId) -> Option<EventId> {
        self.last_entity.get(&e).copied()
    }

    /// Get an event by ID.
    pub fn get(&self, id: EventId) -> Option<&Event> {
        match self.by_id.get(&id) {
            Some(&i) => self.events.get(i).filter(|e| e.id == id),
            None => self.events.iter().find(|e| e.id == id),
        }
    }

    /// Get a mutable reference to an event.
    pub fn get_mut(&mut self, id: EventId) -> Option<&mut Event> {
        if let Some(&i) = self.by_id.get(&id) {
            if self.events.get(i).map_or(false, |e| e.id == id) { return self.events.get_mut(i); }
        }
        self.events.iter_mut().find(|e| e.id == id)
    }

    /// Get all events at a specific date.
    pub fn events_at_date(&self, date: &Date) -> Vec<&Event> {
        self.by_date.get(date)
            .map(|ids| ids.iter().filter_map(|id| self.get(*id)).collect())
            .unwrap_or_default()
    }

    /// Get all events at a specific tile.
    pub fn events_at_location(&self, x: usize, y: usize) -> Vec<&Event> {
        self.by_location.get(&(x, y))
            .map(|ids| ids.iter().filter_map(|id| self.get(*id)).collect())
            .unwrap_or_default()
    }

    /// Get all major events.
    pub fn major_events(&self) -> Vec<&Event> {
        self.events.iter().filter(|e| e.is_major).collect()
    }

    /// Get events in a year range.
    pub fn events_in_range(&self, start: &Date, end: &Date) -> Vec<&Event> {
        self.events.iter()
            .filter(|e| e.date >= *start && e.date <= *end)
            .collect()
    }

    /// Total number of events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Link a triggered event to its cause.
    pub fn link_cause_effect(&mut self, cause_id: EventId, effect_id: EventId) {
        if let Some(cause) = self.get_mut(cause_id) {
            if !cause.triggered_events.contains(&effect_id) {
                cause.triggered_events.push(effect_id);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::events::types::{Event, EventType, EventOutcome};
    use crate::seasons::Season;
    use crate::history::FactionId;

    #[test]
    fn test_chronicle_record_and_query() {
        let mut chronicle = Chronicle::new();
        let date = Date::new(100, Season::Spring);

        let event = Event::new(
            EventId(0), EventType::FactionFounded, date,
            "Founding of Ironhold".to_string(),
            "The dwarves founded Ironhold.".to_string(),
        ).at_location(50, 30)
         .with_faction(FactionId(0));

        chronicle.record(event);

        assert_eq!(chronicle.len(), 1);
        assert_eq!(chronicle.events_at_date(&date).len(), 1);
        assert_eq!(chronicle.events_at_location(50, 30).len(), 1);
        assert_eq!(chronicle.major_events().len(), 1);
    }

    #[test]
    fn test_chronicle_causality_linking() {
        let mut chronicle = Chronicle::new();

        let e1 = Event::new(
            EventId(0), EventType::Assassination,
            Date::new(200, Season::Summer),
            "Assassination".to_string(), "".to_string(),
        );
        let e2 = Event::new(
            EventId(1), EventType::WarDeclared,
            Date::new(200, Season::Autumn),
            "War".to_string(), "".to_string(),
        ).caused_by(EventId(0));

        chronicle.record(e1);
        chronicle.record(e2);
        chronicle.link_cause_effect(EventId(0), EventId(1));

        let cause = chronicle.get(EventId(0)).unwrap();
        assert!(cause.triggered_events.contains(&EventId(1)));
    }
}
