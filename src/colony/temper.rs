//! Experience shapes character: horror, courage and contentment change who settlers are.
//!
//! The idea is Dwarf Fortress's personality change through experience: dwarves who live through
//! enough horror grow jaded (death and terror no longer stress them as they did), and what they
//! go through moves their facets. Here `feel` counts what each settler goes through
//! (`Mind::horrors` for the deep, the dead, a ghost, a beast's eyes; `Mind::braved` for saving
//! someone or slaying a beast), and at dawn (`reckon_temper`) the day leaves its mark: each
//! horror raises anxiety by two (to 95 at most); each brave act raises bravery by three; thirty
//! days content in a row raise cheer by two. A facet that crosses into a new band is said in the
//! log ("Since the night of day 52, Ord starts at every shadow"). Eight horrors and a settler is
//! jaded: from then on the deep, the dead and grief weigh half as much (`Mind::jaded`), said once.

use super::*;
use crate::persona::Facet;

/// The band a facet value reads in (as `persona.rs` words them).
fn band(v: u8) -> u8 { match v { 0..=9 => 0, 10..=24 => 1, 25..=75 => 2, 76..=90 => 3, _ => 4 } }

impl Colony {
    /// Dawn: yesterday's experience settles into character.
    pub(crate) fn reckon_temper(&mut self) {
        let day = self.clock.day();
        for i in 0..self.settlers.len() {
            if !self.settlers[i].alive { continue; }
            // (Infants are not yet who they will be.)
            if self.settlers[i].past.as_ref().map_or(false, |p| p.age < 4) { self.settlers[i].mind.horrors_today = 0; self.settlers[i].mind.braved_today = 0; continue; }
            let (h, b) = (self.settlers[i].mind.horrors_today, self.settlers[i].mind.braved_today);
            self.settlers[i].mind.horrors_today = 0;
            self.settlers[i].mind.braved_today = 0;
            let name = self.settlers[i].name.clone();
            let they = if self.settlers[i].persona.female { "she" } else { "he" };
            // Horror: anxiety rises.
            if h > 0 && !self.settlers[i].mind.jaded {
                let before = self.settlers[i].persona.facet(Facet::Anxiety);
                let after = (before as u32 + 2 * h).min(95) as u8;
                self.settlers[i].persona.facets[Facet::Anxiety as usize] = after;
                if band(after) > band(before) && band(after) >= 3 {
                    self.note(format!("Since what {} saw on day {}, {} starts at every shadow.", they, day.saturating_sub(1), name));
                }
                self.settlers[i].mind.horrors += h;
                if self.settlers[i].mind.horrors >= 8 {
                    self.settlers[i].mind.jaded = true;
                    self.note(format!("{} has seen too much to be shaken now: the dead and the dark weigh on {} no more than the weather.", name, if self.settlers[i].persona.female { "her" } else { "him" }));
                }
            }
            // Courage: bravery rises.
            if b > 0 {
                let before = self.settlers[i].persona.facet(Facet::Bravery);
                let after = (before as u32 + 3 * b).min(100) as u8;
                self.settlers[i].persona.facets[Facet::Bravery as usize] = after;
                if band(after) > band(before) && band(after) >= 3 {
                    self.note(format!("{} has found courage: {} will face what comes now.", name, they));
                }
            }
            // Contentment: cheer rises after thirty good days.
            if self.settlers[i].mind.stress < -0.4 { self.settlers[i].mind.content_days += 1; } else { self.settlers[i].mind.content_days = 0; }
            if self.settlers[i].mind.content_days >= 30 {
                self.settlers[i].mind.content_days = 0;
                let before = self.settlers[i].persona.facet(Facet::Cheer);
                let after = (before + 2).min(100);
                self.settlers[i].persona.facets[Facet::Cheer as usize] = after;
                if band(after) > band(before) && band(after) >= 3 { self.note(format!("Life in the camp suits {}: {} is quicker to laugh than {} was.", name, they, they)); }
            }
        }
    }
}
