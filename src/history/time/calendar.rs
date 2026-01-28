//! Calendar system using seasonal time steps.
//!
//! Each year has 4 seasons. The simulation advances one season at a time.
//! Reuses the existing `Season` enum from `crate::seasons`.

use std::fmt;
use serde::{Serialize, Deserialize};
use crate::seasons::Season;

/// A specific date in the world calendar (year + season).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Date {
    pub year: u32,
    pub season: Season,
}

impl Date {
    pub fn new(year: u32, season: Season) -> Self {
        Self { year, season }
    }

    /// The very first date: Year 1, Spring.
    pub fn origin() -> Self {
        Self { year: 1, season: Season::Spring }
    }

    /// Advance to the next season (mutates in place).
    pub fn advance(&mut self) {
        match self.season {
            Season::Spring => self.season = Season::Summer,
            Season::Summer => self.season = Season::Autumn,
            Season::Autumn => self.season = Season::Winter,
            Season::Winter => {
                self.season = Season::Spring;
                self.year += 1;
            }
        }
    }

    /// Return the next date without mutating.
    pub fn next(&self) -> Self {
        let mut copy = *self;
        copy.advance();
        copy
    }

    /// Total number of seasons from the origin (Year 1 Spring = 0).
    pub fn total_seasons(&self) -> i64 {
        (self.year as i64 - 1) * 4 + self.season as i64
    }

    /// Number of seasons between two dates (positive if self is later).
    pub fn seasons_since(&self, other: &Date) -> i64 {
        self.total_seasons() - other.total_seasons()
    }

    /// Number of full years between two dates.
    pub fn years_since(&self, other: &Date) -> i32 {
        self.year as i32 - other.year as i32
    }

    /// Check if this date falls in a given year.
    pub fn is_in_year(&self, year: u32) -> bool {
        self.year == year
    }

    /// Fractional season offset within a year (Spring=0.0, Summer=0.25, Autumn=0.5, Winter=0.75).
    pub fn season_fraction(&self) -> f32 {
        match self.season {
            Season::Spring => 0.0,
            Season::Summer => 0.25,
            Season::Autumn => 0.5,
            Season::Winter => 0.75,
        }
    }
}

impl Ord for Date {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.total_seasons().cmp(&other.total_seasons())
    }
}

impl PartialOrd for Date {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl fmt::Display for Date {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let season_name = match self.season {
            Season::Spring => "Spring",
            Season::Summer => "Summer",
            Season::Autumn => "Autumn",
            Season::Winter => "Winter",
        };
        write!(f, "{} of Year {}", season_name, self.year)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_date_advance() {
        let mut d = Date::origin();
        assert_eq!(d.year, 1);
        assert_eq!(d.season, Season::Spring);

        d.advance();
        assert_eq!(d.season, Season::Summer);
        assert_eq!(d.year, 1);

        d.advance();
        assert_eq!(d.season, Season::Autumn);

        d.advance();
        assert_eq!(d.season, Season::Winter);

        d.advance();
        assert_eq!(d.season, Season::Spring);
        assert_eq!(d.year, 2);
    }

    #[test]
    fn test_seasons_since() {
        let a = Date::new(1, Season::Spring);
        let b = Date::new(2, Season::Spring);
        assert_eq!(b.seasons_since(&a), 4);

        let c = Date::new(1, Season::Winter);
        assert_eq!(c.seasons_since(&a), 3);
    }

    #[test]
    fn test_date_ordering() {
        let a = Date::new(1, Season::Spring);
        let b = Date::new(1, Season::Summer);
        let c = Date::new(2, Season::Spring);

        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
    }

    #[test]
    fn test_total_seasons() {
        let origin = Date::origin();
        assert_eq!(origin.total_seasons(), 0);

        let y2_spring = Date::new(2, Season::Spring);
        assert_eq!(y2_spring.total_seasons(), 4);

        let y1_winter = Date::new(1, Season::Winter);
        assert_eq!(y1_winter.total_seasons(), 3);
    }

    #[test]
    fn test_display() {
        let d = Date::new(247, Season::Winter);
        assert_eq!(format!("{}", d), "Winter of Year 247");
    }
}
