//! Chronicle: the complete event log with indexing.

use std::collections::{BTreeMap, HashMap};
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
}

impl Chronicle {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record a new event.
    pub fn record(&mut self, event: Event) {
        let id = event.id;
        let date = event.date;
        let location = event.location;

        // Index by date
        self.by_date.entry(date).or_default().push(id);

        // Index by location
        if let Some(loc) = location {
            self.by_location.entry(loc).or_default().push(id);
        }

        self.events.push(event);
    }

    /// Get an event by ID.
    pub fn get(&self, id: EventId) -> Option<&Event> {
        self.events.iter().find(|e| e.id == id)
    }

    /// Get a mutable reference to an event.
    pub fn get_mut(&mut self, id: EventId) -> Option<&mut Event> {
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
