//! Historical timeline and era tracking.
//!
//! Eras are named periods of history defined by major events.

use serde::{Serialize, Deserialize};
use super::calendar::Date;
use crate::history::{EraId, EventId};

/// A named historical era (e.g., "The Age of Strife").
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Era {
    pub id: EraId,
    pub name: String,
    pub start: Date,
    pub end: Option<Date>,
    pub defining_events: Vec<EventId>,
}

impl Era {
    pub fn new(id: EraId, name: String, start: Date) -> Self {
        Self {
            id,
            name,
            start,
            end: None,
            defining_events: Vec::new(),
        }
    }

    /// End this era at the given date.
    pub fn close(&mut self, end: Date) {
        self.end = Some(end);
    }

    /// Duration in years (None if era is still ongoing).
    pub fn duration_years(&self) -> Option<u32> {
        self.end.map(|end| end.year.saturating_sub(self.start.year))
    }

    /// Whether this era is still active.
    pub fn is_active(&self) -> bool {
        self.end.is_none()
    }

    /// Check if a date falls within this era.
    pub fn contains(&self, date: &Date) -> bool {
        if *date < self.start {
            return false;
        }
        match self.end {
            Some(end) => *date <= end,
            None => true,
        }
    }
}

/// Manages the sequence of historical eras.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Timeline {
    pub eras: Vec<Era>,
}

impl Timeline {
    pub fn new() -> Self {
        Self { eras: Vec::new() }
    }

    /// Start a new era, closing the previous one if active.
    pub fn begin_era(&mut self, era: Era) {
        // Close the current era if one is active
        if let Some(current) = self.eras.last_mut() {
            if current.is_active() {
                current.close(era.start);
            }
        }
        self.eras.push(era);
    }

    /// Get the currently active era.
    pub fn current_era(&self) -> Option<&Era> {
        self.eras.last().filter(|e| e.is_active())
    }

    /// Get the era that contains a given date.
    pub fn era_at(&self, date: &Date) -> Option<&Era> {
        self.eras.iter().find(|e| e.contains(date))
    }

    /// Total number of eras.
    pub fn era_count(&self) -> usize {
        self.eras.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::seasons::Season;

    #[test]
    fn test_era_lifecycle() {
        let id = EraId(0);
        let start = Date::new(1, Season::Spring);
        let mut era = Era::new(id, "The Dawn Age".to_string(), start);

        assert!(era.is_active());
        assert_eq!(era.duration_years(), None);

        let end = Date::new(100, Season::Autumn);
        era.close(end);

        assert!(!era.is_active());
        assert_eq!(era.duration_years(), Some(99));
    }

    #[test]
    fn test_era_contains() {
        let era = Era {
            id: EraId(0),
            name: "Test Era".to_string(),
            start: Date::new(10, Season::Spring),
            end: Some(Date::new(20, Season::Winter)),
            defining_events: vec![],
        };

        assert!(!era.contains(&Date::new(9, Season::Winter)));
        assert!(era.contains(&Date::new(10, Season::Spring)));
        assert!(era.contains(&Date::new(15, Season::Summer)));
        assert!(era.contains(&Date::new(20, Season::Winter)));
        assert!(!era.contains(&Date::new(21, Season::Spring)));
    }

    #[test]
    fn test_timeline() {
        let mut timeline = Timeline::new();

        let era1 = Era::new(EraId(0), "First Age".to_string(), Date::new(1, Season::Spring));
        timeline.begin_era(era1);
        assert!(timeline.current_era().is_some());
        assert_eq!(timeline.current_era().unwrap().name, "First Age");

        let era2 = Era::new(EraId(1), "Second Age".to_string(), Date::new(100, Season::Spring));
        timeline.begin_era(era2);

        // First era should now be closed
        assert!(!timeline.eras[0].is_active());
        assert_eq!(timeline.eras[0].end, Some(Date::new(100, Season::Spring)));

        // Second era is active
        assert!(timeline.current_era().is_some());
        assert_eq!(timeline.current_era().unwrap().name, "Second Age");
        assert_eq!(timeline.era_count(), 2);
    }
}
